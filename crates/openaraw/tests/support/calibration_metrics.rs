//! Q-TOF calibration metrics.
//!
//! Ground truth comes only from inside the bundle: each Q-TOF `MSScan.bin`
//! record stores the instrument's own `BasePeakMZ` and `TIC`, and
//! reference-mass ions have m/z that follow from their elemental formulas.
//!
//! `BasePeakMZ` is computed with the bundle's `DefaultMassCal.xml`
//! calibration, before the per-scan reference-mass correction stored in
//! `MSMassCal.bin` (see `docs/format/05-msmasscal.md`). So:
//!
//! - `formula` decodes the base peak with the `DefaultMassCal.xml` entry for
//!   the scan's `CalibrationID`. It checks the calibration formula and is
//!   expected to match `BasePeakMZ` to a small fraction of a ppm.
//! - `after` decodes it the way the reader does (the per-scan record where
//!   present). Its distance from `BasePeakMZ` is the reference-mass
//!   correction, not an error.
//! - Reference-mass ions measure the reader's absolute accuracy.
//!
//! In bundles with profile data, `TIC`, `BasePeakValue` and `BasePeakMZ`
//! describe the profile spectrum (its intensity sum, its most intense
//! point and that point's m/z); otherwise they describe the centroids.
//! Both comparisons are made where the data exists, and each test picks
//! the one that applies.
//!
//! Shared by `tests/qtof_calibration.rs` and the `qtof_calibration_check`
//! example (which includes this file by path).

use openaraw::raw::calibration::{CalibrationSource, TofCalibration};
use openaraw::raw::defaultmasscal;
use openaraw::raw::mspeak::decode_peak_block;
use openaraw::raw::msprofile::decode_profile_block;
use openaraw::raw::read_bytes;
use openaraw::Reader;
use std::collections::HashMap;
use std::fmt;

/// Reference-mass ions commonly infused on Q-TOF instruments, with m/z
/// computed from elemental formulas (monoisotopic masses, proton or
/// electron mass applied).
pub const REFERENCE_IONS: &[(&str, f64)] = &[
    ("purine [M+H]+ C5H5N4", 121.050873),
    ("HP-0921 [M+H]+ C18H19F24N3O6P3", 922.009798),
    ("HP-1221 [M+H]+ C24H19F36N3O6P3", 1221.990637),
    ("TFA [M-H]- C2F3O2", 112.985587),
    ("HP-0921+TFA [M+TFA-H]- C20H18F27N3O8P3", 1033.988109),
];

/// Window (ppm) for matching reference ions in MS1 centroids.
pub const REFERENCE_WINDOW_PPM: f64 = 20.0;

/// A reference ion is reported when it is found in at least this fraction
/// of MS1 scans (otherwise nearby matches are likely chance hits).
pub const REFERENCE_MIN_FRACTION: f64 = 0.5;

/// Distribution summary of a set of samples.
#[derive(Debug, Clone, Default)]
pub struct Summary {
    pub n: usize,
    pub median: f64,
    pub median_abs: f64,
    pub p95_abs: f64,
}

impl Summary {
    pub fn of(values: &[f64]) -> Self {
        if values.is_empty() {
            return Summary::default();
        }
        let mut signed = values.to_vec();
        signed.sort_by(f64::total_cmp);
        let mut abs: Vec<f64> = values.iter().map(|v| v.abs()).collect();
        abs.sort_by(f64::total_cmp);
        Summary {
            n: values.len(),
            median: percentile(&signed, 50.0),
            median_abs: percentile(&abs, 50.0),
            p95_abs: percentile(&abs, 95.0),
        }
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.n == 0 {
            return write!(f, "n=0");
        }
        write!(
            f,
            "n={} median={:.4} median|.|={:.4} p95|.|={:.4}",
            self.n, self.median, self.median_abs, self.p95_abs
        )
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn ppm(observed: f64, expected: f64) -> f64 {
    (observed - expected) / expected * 1e6
}

/// Index of the most intense point.
fn apex(intensity: &[f32]) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, &v) in intensity.iter().enumerate() {
        if best.is_none_or(|(_, b)| v > b) {
            best = Some((i, v));
        }
    }
    best.filter(|&(_, v)| v > 0.0).map(|(i, _)| i)
}

/// Calibration metrics for one bundle. All `*_bp_ppm` values compare the
/// most intense centroid (or profile peak) with the stored `BasePeakMZ`.
#[derive(Debug, Default)]
pub struct Report {
    /// origin/main behaviour (`x / 100` reported as m/z).
    pub before_bp_ppm: Summary,
    /// `Traditional` step only, with the reader's per-scan calibration.
    pub traditional_bp_ppm: Summary,
    /// The reader's output.
    pub after_bp_ppm: Summary,
    /// Full formula with the `DefaultMassCal.xml` calibration that
    /// `BasePeakMZ` was computed with.
    pub formula_bp_ppm: Summary,
    /// The reader's profile output: m/z of the most intense profile point.
    pub profile_bp_ppm: Summary,
    /// Most intense profile point with the `DefaultMassCal.xml`
    /// calibration.
    pub profile_formula_bp_ppm: Summary,
    /// Summed centroid intensity / `TIC`.
    pub centroid_tic_ratio: Summary,
    /// Summed profile intensity / `TIC`.
    pub profile_tic_ratio: Summary,
    /// Profile apex intensity / stored `BasePeakValue`.
    pub profile_apex_ratio: Summary,
    /// Reference ions found in enough MS1 scans: name, m/z, hit fraction,
    /// ppm of the reader's output.
    pub reference_ions: Vec<(&'static str, f64, f64, Summary)>,
    pub scans: usize,
    pub ms1_scans: usize,
    pub record_calibrated: usize,
    pub default_calibrated: usize,
    pub uncalibrated_scans: usize,
    pub centroid_failures: usize,
    pub profile_failures: usize,
}

/// `DefaultMassCal.xml` calibrations by `DefaultCalibrationID`; empty if
/// the file is missing or invalid.
fn default_calibrations(reader: &Reader) -> HashMap<i32, TofCalibration> {
    let path = reader.dir().join("AcqData").join("DefaultMassCal.xml");
    defaultmasscal::parse(&path)
        .map(|cals| {
            cals.iter()
                .filter_map(|c| c.to_tof().ok().map(|t| (c.id, t)))
                .collect()
        })
        .unwrap_or_default()
}

/// Evaluate every scan. `profile` controls whether profile blocks are
/// decoded (they dominate the run time on large bundles).
pub fn evaluate(reader: &Reader, profile: bool) -> Report {
    let acq = reader.dir().join("AcqData");
    let peak_path = acq.join("MSPeak.bin");
    let profile_path = acq.join("MSProfile.bin");
    let defaults = default_calibrations(reader);

    let mut before = Vec::new();
    let mut traditional = Vec::new();
    let mut after = Vec::new();
    let mut formula = Vec::new();
    let mut profile_bp = Vec::new();
    let mut profile_formula = Vec::new();
    let mut centroid_tic = Vec::new();
    let mut profile_tic = Vec::new();
    let mut profile_apex = Vec::new();
    let mut reference: Vec<Vec<f64>> = vec![Vec::new(); REFERENCE_IONS.len()];
    let mut report = Report::default();

    for (index, rec) in reader.msscan().records.iter().enumerate() {
        report.scans += 1;
        if rec.ms_level == 1 {
            report.ms1_scans += 1;
        }
        let Some(cal) = reader.calibration(index) else {
            report.uncalibrated_scans += 1;
            continue;
        };
        match cal.source {
            CalibrationSource::MassCalRecord => report.record_calibrated += 1,
            CalibrationSource::DefaultMassCal => report.default_calibrated += 1,
            other => panic!("unexpected calibration source {other:?}"),
        }
        let stored_bp = rec.base_peak_mz.filter(|v| *v > 0.0);
        let stored_tic = rec.tic.filter(|v| *v > 0.0);

        if let Some(params) = &rec.centroid_params {
            let block = read_bytes(&peak_path, params.offset, params.byte_count as usize);
            match block.and_then(|b| decode_peak_block(&b, params, Some(cal)).map(|s| (b, s))) {
                Ok((bytes, spec)) => {
                    if let (Some(bp), Some(top)) = (stored_bp, apex(&spec.intensity)) {
                        let t = f64::from_le_bytes(bytes[top * 8..top * 8 + 8].try_into().unwrap());
                        before.push(ppm(t / 100.0, bp));
                        traditional.push(ppm((cal.a * (t - cal.t0)).powi(2), bp));
                        after.push(ppm(spec.mz[top], bp));
                        if let Some(d) = rec.calibration_id.and_then(|id| defaults.get(&id)) {
                            formula.push(ppm(d.mz(t), bp));
                        }
                    }
                    if let Some(tic) = stored_tic {
                        let sum: f64 = spec.intensity.iter().map(|&v| v as f64).sum();
                        centroid_tic.push(sum / tic);
                    }
                    if rec.ms_level == 1 {
                        match_reference_ions(&spec.mz, &mut reference);
                    }
                }
                Err(err) => {
                    eprintln!("scanId={}: centroid decode failed: {err}", rec.scan_id);
                    report.centroid_failures += 1;
                }
            }
        }

        if !profile {
            continue;
        }
        if let Some(params) = &rec.profile_params {
            let decoded = read_bytes(&profile_path, params.offset, params.byte_count as usize)
                .and_then(|b| decode_profile_block(&b, params, cal));
            match decoded {
                Ok(spec) => {
                    if let (Some(bp), Some(top)) = (stored_bp, apex(&spec.intensity)) {
                        profile_bp.push(ppm(spec.mz[top], bp));
                        // Decode the same block with the default calibration
                        // (identical when the reader already uses it).
                        let default = rec.calibration_id.and_then(|id| defaults.get(&id));
                        if let Some(d) = default {
                            let mz = if cal.source == CalibrationSource::DefaultMassCal {
                                spec.mz[top]
                            } else {
                                read_bytes(&profile_path, params.offset, params.byte_count as usize)
                                    .and_then(|b| decode_profile_block(&b, params, d))
                                    .map(|s| s.mz[top])
                                    .unwrap_or(f64::NAN)
                            };
                            profile_formula.push(ppm(mz, bp));
                        }
                        if let Some(v) = rec.base_peak_intensity.filter(|v| *v > 0.0) {
                            profile_apex.push(spec.intensity[top] as f64 / v);
                        }
                    }
                    if let Some(tic) = stored_tic {
                        let sum: f64 = spec.intensity.iter().map(|&v| v as f64).sum();
                        profile_tic.push(sum / tic);
                    }
                }
                Err(err) => {
                    eprintln!("scanId={}: profile decode failed: {err}", rec.scan_id);
                    report.profile_failures += 1;
                }
            }
        }
    }

    report.before_bp_ppm = Summary::of(&before);
    report.traditional_bp_ppm = Summary::of(&traditional);
    report.after_bp_ppm = Summary::of(&after);
    report.formula_bp_ppm = Summary::of(&formula);
    report.profile_bp_ppm = Summary::of(&profile_bp);
    report.profile_formula_bp_ppm = Summary::of(&profile_formula);
    report.centroid_tic_ratio = Summary::of(&centroid_tic);
    report.profile_tic_ratio = Summary::of(&profile_tic);
    report.profile_apex_ratio = Summary::of(&profile_apex);
    let ms1 = report.ms1_scans.max(1) as f64;
    report.reference_ions = REFERENCE_IONS
        .iter()
        .zip(reference)
        .map(|(&(name, mz), hits)| (name, mz, hits.len() as f64 / ms1, Summary::of(&hits)))
        .filter(|(_, _, fraction, _)| *fraction >= REFERENCE_MIN_FRACTION)
        .collect();
    report
}

fn match_reference_ions(mz: &[f64], hits: &mut [Vec<f64>]) {
    for (&(_, target), out) in REFERENCE_IONS.iter().zip(hits.iter_mut()) {
        let nearest = mz
            .iter()
            .map(|&m| ppm(m, target))
            .min_by(|a, b| a.abs().total_cmp(&b.abs()));
        if let Some(d) = nearest.filter(|d| d.abs() <= REFERENCE_WINDOW_PPM) {
            out.push(d);
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "scans={} ms1={} calibration: record={} default_xml={} none={} \
             centroid_failures={} profile_failures={}",
            self.scans,
            self.ms1_scans,
            self.record_calibrated,
            self.default_calibrated,
            self.uncalibrated_scans,
            self.centroid_failures,
            self.profile_failures
        )?;
        let rows: [(&str, &Summary); 9] = [
            ("before (x/100) BP ppm", &self.before_bp_ppm),
            ("traditional only BP ppm", &self.traditional_bp_ppm),
            ("after (reader) BP ppm", &self.after_bp_ppm),
            ("formula (default cal) BP ppm", &self.formula_bp_ppm),
            ("profile (reader) BP ppm", &self.profile_bp_ppm),
            ("profile formula BP ppm", &self.profile_formula_bp_ppm),
            ("centroid sum/TIC", &self.centroid_tic_ratio),
            ("profile sum/TIC", &self.profile_tic_ratio),
            ("profile apex/BasePeakValue", &self.profile_apex_ratio),
        ];
        for (name, s) in rows {
            writeln!(f, "{name:30} {s}")?;
        }
        for (name, mz, fraction, s) in &self.reference_ions {
            writeln!(
                f,
                "reference {name} {mz:.6} in {:.0}% of MS1: ppm {s}",
                fraction * 100.0
            )?;
        }
        Ok(())
    }
}
