//! Run-length profile blocks (acquisition software 10.1, `PXD044639`)
//! checked against the summary stored with every scan in `MSScan.bin`.
//! Needs the research corpus (`OPENARAW_CORPUS`); skips otherwise, fails
//! under `REQUIRE_CORPUS=1`. See `docs/format/04-msprofile.md`.
//!
//! These bundles hold profile data only, and their `TIC`,
//! `BasePeakValue` and `BasePeakMZ` describe it. For every scan:
//!
//! - the decoded intensities sum to `TIC` exactly and their maximum is
//!   `BasePeakValue` exactly (both are integer counts);
//! - a point holding `BasePeakValue` sits within `FORMULA_MAX_PPM` of
//!   `BasePeakMZ` when converted with the `DefaultMassCal.xml` calibration
//!   `BasePeakMZ` was computed with (see `docs/format/05-msmasscal.md`).
//!
//! A run over all 13 bundles (192,503 scans) matched `TIC` and
//! `BasePeakValue` exactly on every scan.

use openaraw::raw::calibration::TofCalibration;
use openaraw::raw::defaultmasscal;
use openaraw::raw::msprofile::ProfileAxis;
use openaraw::raw::msprofile::{decode_run_length, ProfileEncoding, AXIS_HEADER_SIZE};
use openaraw::raw::read_bytes;
use openaraw::Reader;
use openmassspec_core::{ScanMode, SpectrumSource};
use std::collections::HashMap;
use std::path::Path;

mod support;
use support::fixture;

/// Same limit as the `p95` formula check in `tests/qtof_calibration.rs`.
const FORMULA_MAX_PPM: f64 = 0.01;

fn ppm(observed: f64, expected: f64) -> f64 {
    (observed - expected) / expected * 1e6
}

fn default_calibrations(dir: &Path) -> HashMap<i32, TofCalibration> {
    defaultmasscal::parse(&dir.join("AcqData").join("DefaultMassCal.xml"))
        .expect("DefaultMassCal.xml")
        .iter()
        .map(|c| (c.id, c.to_tof().expect("default calibration")))
        .collect()
}

/// Decode every profile block of a bundle and compare it with the stored
/// summary. Returns the number of scans checked.
fn check_blocks(bundle: &str) -> Option<usize> {
    let path = fixture("PXD044639 Q-TOF", None, &format!("PXD044639/{bundle}"))?;
    let reader = Reader::open(&path).expect("open bundle");
    let defaults = default_calibrations(&path);
    let profile_path = path.join("AcqData").join("MSProfile.bin");

    let mut worst_ppm = 0.0f64;
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for rec in &reader.msscan().records {
        let params = rec.profile_params.as_ref().expect("profile block");
        let block = read_bytes(&profile_path, params.offset, params.byte_count as usize)
            .expect("read block");
        assert_eq!(
            ProfileEncoding::detect(&block),
            ProfileEncoding::RunLength,
            "scanId={}",
            rec.scan_id
        );
        let axis = ProfileAxis::from_bytes(&block[..AXIS_HEADER_SIZE]).expect("axis");
        let intensity =
            match decode_run_length(&block[AXIS_HEADER_SIZE..], params.point_count as usize) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("scanId={}: {err}", rec.scan_id));
                    continue;
                }
            };
        checked += 1;

        let tic: i64 = intensity.iter().sum();
        let top = intensity.iter().copied().max().unwrap_or(0);
        let stored_tic = rec.tic.expect("TIC");
        let stored_top = rec.base_peak_intensity.expect("BasePeakValue");
        if tic as f64 != stored_tic || top as f64 != stored_top {
            failures.push(format!(
                "scanId={}: sum {tic} / max {top}, stored TIC {stored_tic} / \
                 BasePeakValue {stored_top}",
                rec.scan_id
            ));
            continue;
        }

        let stored_mz = rec.base_peak_mz.expect("BasePeakMZ");
        let cal = &defaults[&rec.calibration_id.expect("CalibrationID")];
        let best = intensity
            .iter()
            .enumerate()
            .filter(|&(_, &v)| v == top)
            .map(|(i, _)| ppm(cal.mz(axis.x0 + i as f64 * axis.dx), stored_mz))
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
            .expect("apex");
        worst_ppm = worst_ppm.max(best.abs());
        if best.abs() > FORMULA_MAX_PPM {
            failures.push(format!(
                "scanId={}: base peak {best:.6} ppm from BasePeakMZ {stored_mz}",
                rec.scan_id
            ));
        }
    }
    eprintln!(
        "{}: {checked} of {} scans decoded, worst base peak |ppm| {worst_ppm:.6}",
        path.display(),
        reader.msscan().records.len()
    );
    assert!(
        failures.is_empty(),
        "{} scans failed, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
    assert_eq!(checked, reader.msscan().records.len());
    assert!(checked > 0);
    Some(checked)
}

macro_rules! bundle_tests {
    ($($name:ident => $bundle:literal,)*) => {
        $(
            #[test]
            fn $name() {
                check_blocks($bundle);
            }
        )*
    };
}

bundle_tests! {
    pxd044639_a1fu => "A1FU.d",
    pxd044639_a4fo => "A4FO.d",
    pxd044639_a5fu => "A5FU.d",
    pxd044639_b15fo => "B15FO.d",
    pxd044639_b2fu => "B2FU.d",
    pxd044639_b4fu => "B4FU.d",
    pxd044639_c1fu => "C1FU.d",
    pxd044639_c2fo => "C2FO.d",
    pxd044639_c3fo => "C3FO.d",
    pxd044639_c4fu => "C4FU.d",
    pxd044639_c5fu => "C5FU.d",
    pxd044639_d1fo => "D1FO.d",
    pxd044639_d1fu => "D1FU.d",
}

/// Through the reader: every scan comes out as a non-empty profile
/// spectrum whose intensities sum to the stored `TIC`, with no decode
/// failures.
#[test]
fn pxd044639_reader_output() {
    let Some(path) = fixture("PXD044639 Q-TOF", None, "PXD044639/A1FU.d") else {
        return;
    };
    let mut reader = Reader::open(&path).expect("open bundle");
    let mut spectra = 0usize;
    for spectrum in reader.iter_spectra() {
        let spectrum = spectrum.expect("spectrum decodes");
        spectra += 1;
        assert_eq!(spectrum.scan_mode, Some(ScanMode::Profile));
        assert!(!spectrum.mz.is_empty(), "{}", spectrum.native_id);
        assert_eq!(spectrum.mz.len(), spectrum.intensity.len());
        let sum: f64 = spectrum.intensity.iter().map(|&v| v as f64).sum();
        assert_eq!(
            Some(sum),
            spectrum.total_ion_current,
            "{}",
            spectrum.native_id
        );
    }
    assert_eq!(spectra, reader.msscan().records.len());
    assert_eq!(reader.profile_decode_failures(), 0);
    assert_eq!(reader.centroid_decode_failures(), 0);
}
