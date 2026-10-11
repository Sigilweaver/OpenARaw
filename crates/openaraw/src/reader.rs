//! High-level reader for an Agilent `.d` bundle directory.

use openmassspec_core::{
    Analyzer, ChromatogramIter, ChromatogramRecord, CvTerm, PrecursorInfo, RunMetadata, ScanMode,
    SpectrumIter, SpectrumRecord, SpectrumSource,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::raw::calibration::{CalibrationSource, TofCalibration, POLYNOMIAL_VALUES};
use crate::raw::defaultmasscal::DefaultCalibration;
use crate::raw::metadata::{parse_acquired_time, parse_devices_xml, DeviceInfo};
use crate::raw::mspeak::decode_peak_block;
use crate::raw::msprofile::decode_profile_block;
use crate::raw::msscan::{is_qtof_stride, MSScan, ScanRecord};

/// Reader for one Agilent MassHunter `.d` bundle. Implements
/// [`SpectrumSource`].
pub struct Reader {
    dir: PathBuf,
    bundle_name: String,
    msscan: MSScan,
    device: Option<DeviceInfo>,
    peak_path: PathBuf,
    profile_path: PathBuf,
    instrument: CvTerm,
    analyzer: Analyzer,
    start_timestamp: Option<String>,
    /// Per-scan Q-TOF calibration, indexed like `msscan.records`. Empty
    /// when the bundle has no usable `MSMassCal.bin`.
    calibrations: Vec<Option<TofCalibration>>,
    profile_decode_failures: AtomicUsize,
    centroid_decode_failures: AtomicUsize,
}

/// Payload kind for decode-failure accounting.
#[derive(Clone, Copy)]
enum PayloadKind {
    Profile,
    Centroid,
}

impl PayloadKind {
    fn name(self) -> &'static str {
        match self {
            PayloadKind::Profile => "profile",
            PayloadKind::Centroid => "centroid",
        }
    }
}

/// Resolve a PSI-MS CV term for an Agilent instrument model read from
/// `Devices.xml`'s `<ModelNumber>` field (e.g. `"G6550A"`). Falls back to
/// the generic `MS:1000490` "Agilent instrument model" term for models
/// without a dedicated PSI-MS entry (e.g. `G6540A`/`G6540B`/`G6550B`,
/// which the CV only tracks at the unsuffixed `6540`/`6550` family level,
/// or not at all).
///
/// Table built from `psi-ms.obo`, restricted to the `is_a: MS:1000490 !
/// Agilent instrument model` subtree, matched against the exact model
/// numbers observed across the 330-bundle validation corpus (see
/// `CORPUS.md` and `docs/format/01-msscan.md`).
fn instrument_cv_for_model(model: &str) -> CvTerm {
    let known: &[(&str, &str, &str)] = &[
        ("G6410A", "MS:1003526", "6410A Triple Quadrupole LC/MS"),
        ("G6530A", "MS:1002786", "6530A Q-TOF LC/MS"),
        ("G6530B", "MS:1002787", "6530B Q-TOF LC/MS"),
        ("G6550A", "MS:1002784", "6550A iFunnel Q-TOF LC/MS"),
    ];
    for (m, acc, name) in known {
        if model.eq_ignore_ascii_case(m) {
            return CvTerm::new(*acc, *name);
        }
    }
    CvTerm::new("MS:1000490", "Agilent instrument model")
}

/// Resolve the run's instrument CV term, preferring the real device
/// identity parsed from `Devices.xml` over the record-stride guess.
/// Falls back to the stride-based guess (generic `MS:1000461` tag) when `Devices.xml` is missing,
/// unparseable, or lacks the fields we need - this is a real condition
/// in the wild (see `docs/format/06-known-limitations.md`), not just a
/// defensive fallback.
fn resolve_instrument(device: Option<&DeviceInfo>, msscan: &MSScan) -> CvTerm {
    if let Some(device) = device {
        return instrument_cv_for_model(&device.model);
    }

    let instrument_name = if is_qtof_stride(msscan.stride) {
        "Agilent Q-TOF"
    } else {
        "Agilent QQQ"
    };
    CvTerm::new("MS:1000461", instrument_name) // generic agilent node; Devices.xml unavailable
}

/// Resolve the mass analyzer type, preferring `Devices.xml`'s `<Name>`
/// field over the record-stride guess.
///
/// The mass spectrometer `<Device>`'s `<Name>` is confirmed (across the
/// 330-bundle validation corpus, see `CORPUS.md` and the module docs on
/// [`parse_devices_xml`]) to be exactly `"QTOF"` (`Type=6`, Q-TOF
/// instruments) or `"TandemQuadrupole"` (`Type=5`, QQQ instruments) - a
/// direct statement of the analyzer family from the acquisition's own
/// metadata, rather than a guess inferred from `MSScan.bin`'s record
/// size. Falls back to the stride-based guess (a Q-TOF stride per
/// [`is_qtof_stride`], i.e. 216, 220 or 284 => TOFMS; any other => TQMS) when
/// `Devices.xml` is missing, unparseable, or names a device outside
/// those two known values - the same real-world condition
/// `resolve_instrument` above falls back for (see
/// `docs/format/06-known-limitations.md`).
fn resolve_analyzer(device: Option<&DeviceInfo>, msscan: &MSScan) -> Analyzer {
    match device.map(|d| d.name.as_str()) {
        Some(name) if name.eq_ignore_ascii_case("QTOF") => return Analyzer::TOFMS,
        Some(name) if name.eq_ignore_ascii_case("TandemQuadrupole") => return Analyzer::TQMS,
        _ => {}
    }

    if is_qtof_stride(msscan.stride) {
        Analyzer::TOFMS
    } else {
        Analyzer::TQMS
    }
}

/// Build the run-level summary chromatograms (TIC and BPC) from an already
/// decoded spectrum stream.
///
/// This is a pure aggregation of data OpenARaw already decodes - it does no
/// additional binary parsing. Each point comes from
/// [`SpectrumRecord::effective_tic`] and
/// [`SpectrumRecord::effective_base_peak`]: the instrument-reported `TIC` /
/// `BasePeakValue` from `MSScan.bin` where the record carries them (Q-TOF),
/// otherwise the sum and maximum of the decoded intensities.
///
/// Only MS1 scans contribute: a survey (MS1) trace is what the "total ion
/// current chromatogram" / "basepeak chromatogram" CV terms describe, and
/// mixing interleaved MS2 scans in would produce a non-chromatographic,
/// RT-duplicated trace. Runs with no MS1 scans (e.g. QQQ MRM acquisitions,
/// which are all MS2) therefore yield no summary chromatograms rather than a
/// misleading one.
///
/// Per-transition SRM/MRM chromatograms are intentionally not produced:
/// OpenARaw decodes only an opaque `mrm_channel_id`, not the Q1/Q3 isolation
/// windows an SRM chromatogram needs, and recovering those would be new
/// binary parsing rather than wiring up already-decoded data.
///
/// `SpectrumRecord::retention_time_sec` is already in seconds, matching
/// `ChromatogramRecord::time_sec`; it is only narrowed to `f32` here.
///
/// The first spectrum that fails to decode is returned as the error: a
/// trace with a missing point would misstate the run.
fn summary_chromatograms<I>(spectra: I) -> openmassspec_core::Result<Vec<ChromatogramRecord>>
where
    I: Iterator<Item = openmassspec_core::Result<SpectrumRecord>>,
{
    let mut time_sec: Vec<f32> = Vec::new();
    let mut tic: Vec<f32> = Vec::new();
    let mut bpc: Vec<f32> = Vec::new();

    for rec in spectra {
        let rec = rec?;
        if rec.ms_level != 1 {
            continue;
        }
        time_sec.push(rec.retention_time_sec as f32);
        tic.push(rec.effective_tic() as f32);
        bpc.push(rec.effective_base_peak().map_or(0.0, |(_, i)| i as f32));
    }

    if time_sec.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![
        ChromatogramRecord::new(0, "TIC", time_sec.clone(), tic)
            .with_chromatogram_type(CvTerm::new("MS:1000235", "total ion current chromatogram")),
        ChromatogramRecord::new(1, "BPC", time_sec, bpc)
            .with_chromatogram_type(CvTerm::new("MS:1000628", "basepeak chromatogram")),
    ])
}

/// Log one problem per scan: the first at warn level, the rest at debug,
/// and return the running count.
fn log_scan_problem(count: &mut usize, bundle: &str, scan_index: usize, msg: &str) {
    if *count == 0 {
        log::warn!("{bundle}: scan index {scan_index}: {msg}");
    } else {
        log::debug!("{bundle}: scan index {scan_index}: {msg}");
    }
    *count += 1;
}

/// The `ValueUseFlags` for an `MSMassCal.bin` record's `Polynomial` values
/// (0 when every coefficient slot is zero). The record does not store
/// them; they come from the `DefaultMassCal.xml` entry whose `Polynomial`
/// values equal the record's to within 1e-12 relative, preferring the
/// entry the scan's `CalibrationID` names.
fn polynomial_flags(
    record: &[f64],
    calibration_id: Option<i32>,
    defaults: &[DefaultCalibration],
) -> crate::Result<u32> {
    let values = record.get(2..).unwrap_or(&[]);
    // With every coefficient slot zero the flags select nothing.
    if values.len() == POLYNOMIAL_VALUES && values[2..].iter().all(|v| *v == 0.0) {
        return Ok(0);
    }
    // The XML holds the values as 15-significant-digit decimal text, so
    // they match the record's doubles to about 1e-15 relative.
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs());
    let matches = |d: &&DefaultCalibration| {
        d.polynomial.len() == values.len()
            && d.polynomial.iter().zip(values).all(|(a, b)| close(*a, *b))
    };
    defaults
        .iter()
        .filter(|d| Some(d.id) == calibration_id)
        .find(matches)
        .or_else(|| defaults.iter().find(matches))
        .map(|d| d.polynomial_flags)
        .ok_or_else(|| {
            crate::Error::Parse(
                "MSMassCal.bin Polynomial values match no DefaultMassCal.xml entry, \
                 so their ValueUseFlags are unknown"
                    .into(),
            )
        })
}

/// Load per-scan Q-TOF calibrations (see `docs/format/05-msmasscal.md`).
/// Returns an empty list for non-Q-TOF bundles.
///
/// Each scan uses its own `MSMassCal.bin` record (record `i` belongs to
/// `MSScan.bin` record `i`), with the `Polynomial` step's `ValueUseFlags`
/// taken from `DefaultMassCal.xml`. When `MSMassCal.bin` is absent the
/// scan uses the `DefaultMassCal.xml` entry for its `CalibrationID`; this
/// is logged at info level. When `MSMassCal.bin` is unreadable, has the
/// wrong record count, or a record is unusable, the affected scans fall
/// back to `DefaultMassCal.xml` with a warning. Scans with no usable
/// calibration at all are logged and get `None`; their payloads are then
/// counted as decode failures rather than reported as m/z.
fn load_calibrations(acq: &Path, msscan: &MSScan, bundle: &str) -> Vec<Option<TofCalibration>> {
    if !is_qtof_stride(msscan.stride) {
        return Vec::new();
    }

    let xml_path = acq.join("DefaultMassCal.xml");
    let defaults = if xml_path.exists() {
        crate::raw::defaultmasscal::parse(&xml_path).unwrap_or_else(|err| {
            log::warn!("{bundle}: {err}");
            Vec::new()
        })
    } else {
        log::warn!("{bundle}: DefaultMassCal.xml not found");
        Vec::new()
    };

    let mascal_path = acq.join("MSMassCal.bin");
    let records = if mascal_path.exists() {
        match crate::raw::msmasscal::parse(&mascal_path) {
            Ok(records) if records.len() == msscan.records.len() => Some(records),
            Ok(records) => {
                log::warn!(
                    "{bundle}: MSMassCal.bin has {} records but MSScan.bin has {} scans; \
                     using DefaultMassCal.xml",
                    records.len(),
                    msscan.records.len()
                );
                None
            }
            Err(err) => {
                log::warn!("{bundle}: MSMassCal.bin: {err}; using DefaultMassCal.xml");
                None
            }
        }
    } else {
        if !defaults.is_empty() {
            log::info!("{bundle}: MSMassCal.bin not found; using DefaultMassCal.xml");
        }
        None
    };

    let default_for = |id: Option<i32>| -> crate::Result<TofCalibration> {
        defaults
            .iter()
            .find(|d| Some(d.id) == id)
            .ok_or_else(|| {
                crate::Error::Parse(format!(
                    "no DefaultMassCal.xml entry for CalibrationID {id:?}"
                ))
            })?
            .to_tof()
    };

    let (mut fallbacks, mut missing) = (0usize, 0usize);
    let calibrations = msscan
        .records
        .iter()
        .enumerate()
        .map(|(i, scan)| {
            if let Some(records) = &records {
                let coefficients = &records[i].coefficients;
                let own = polynomial_flags(coefficients, scan.calibration_id, &defaults).and_then(
                    |flags| {
                        TofCalibration::from_coefficients(
                            coefficients,
                            flags,
                            CalibrationSource::MassCalRecord,
                        )
                    },
                );
                match own {
                    Ok(cal) => return Some(cal),
                    Err(err) => log_scan_problem(
                        &mut fallbacks,
                        bundle,
                        i,
                        &format!("{err}; using DefaultMassCal.xml"),
                    ),
                }
            }
            match default_for(scan.calibration_id) {
                Ok(cal) => Some(cal),
                Err(err) => {
                    log_scan_problem(&mut missing, bundle, i, &format!("no calibration: {err}"));
                    None
                }
            }
        })
        .collect();
    if fallbacks > 0 {
        log::warn!("{bundle}: {fallbacks} scans fell back to DefaultMassCal.xml");
    }
    if missing > 0 {
        log::warn!("{bundle}: {missing} scans have no usable mass calibration");
    }
    calibrations
}

/// Log the scans whose `IonPolarity` value is neither 0 nor 1. Their
/// spectra carry `polarity = None` and the raw value in
/// `extra["openaraw.ion_polarity"]`.
fn warn_unknown_polarity(msscan: &MSScan, bundle: &str) {
    let mut unknown = std::collections::BTreeMap::<i32, usize>::new();
    for rec in &msscan.records {
        if rec.polarity().is_none() {
            *unknown.entry(rec.ion_polarity).or_default() += 1;
        }
    }
    if !unknown.is_empty() {
        log::warn!(
            "{bundle}: unrecognized IonPolarity values (value: scans) {unknown:?}; \
             polarity left unset for those scans"
        );
    }
}

impl Reader {
    pub fn open<P: AsRef<Path>>(dir: P) -> crate::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let acq_data = dir.join("AcqData");

        if !acq_data.exists() {
            return Err(crate::Error::Parse("AcqData directory not found".into()));
        }

        let msscan_path = acq_data.join("MSScan.bin");
        let msscan = MSScan::from_path(&msscan_path)?;

        let bundle_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bundle.d".into());

        // Parsed once and shared: both the run-level instrument CV term and
        // the per-spectrum analyzer type are resolved from the same
        // `Devices.xml` device entry (see `resolve_instrument` and
        // `resolve_analyzer`).
        let device = parse_devices_xml(&acq_data.join("Devices.xml"));
        let instrument = resolve_instrument(device.as_ref(), &msscan);
        let analyzer = resolve_analyzer(device.as_ref(), &msscan);
        let start_timestamp = parse_acquired_time(&acq_data.join("Contents.xml"));
        let calibrations = load_calibrations(&acq_data, &msscan, &bundle_name);
        warn_unknown_polarity(&msscan, &bundle_name);

        Ok(Reader {
            dir,
            bundle_name,
            msscan,
            device,
            peak_path: acq_data.join("MSPeak.bin"),
            profile_path: acq_data.join("MSProfile.bin"),
            instrument,
            analyzer,
            start_timestamp,
            calibrations,
            profile_decode_failures: AtomicUsize::new(0),
            centroid_decode_failures: AtomicUsize::new(0),
        })
    }

    /// The bundle directory passed to [`Reader::open`].
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The bundle directory's file name (e.g. `"sample.d"`), used as the
    /// run's source file name.
    pub fn bundle_name(&self) -> &str {
        &self.bundle_name
    }

    /// The decoded `MSScan.bin` header and scan records. Record `i` is the
    /// spectrum with index `i`.
    pub fn msscan(&self) -> &MSScan {
        &self.msscan
    }

    /// The mass spectrometer entry from `Devices.xml`, when the file is
    /// present and names one.
    pub fn device(&self) -> Option<&DeviceInfo> {
        self.device.as_ref()
    }

    /// Q-TOF flight-time calibration for the scan at `index`, if one could
    /// be loaded (see [`TofCalibration::source`] for where it came from).
    /// Always `None` for QQQ bundles.
    pub fn calibration(&self, index: usize) -> Option<&TofCalibration> {
        self.calibrations.get(index).and_then(Option::as_ref)
    }

    /// Number of scans whose `MSProfile.bin` block could not be read or
    /// decoded so far (cumulative across `iter_spectra` calls). Each such
    /// scan is yielded by `iter_spectra` as an [`crate::Error::Scan`].
    pub fn profile_decode_failures(&self) -> usize {
        self.profile_decode_failures.load(Ordering::Relaxed)
    }

    /// Number of scans whose `MSPeak.bin` block could not be read or
    /// decoded so far (cumulative across `iter_spectra` calls). Each such
    /// scan is yielded by `iter_spectra` as an [`crate::Error::Scan`].
    pub fn centroid_decode_failures(&self) -> usize {
        self.centroid_decode_failures.load(Ordering::Relaxed)
    }

    /// Count a payload read/decode failure and log it. The first failure of
    /// each kind per reader is logged at warn level; the rest at debug, so
    /// a bundle where every scan fails does not flood the log. Returns the
    /// error to yield for the scan.
    fn record_decode_failure(
        &self,
        kind: PayloadKind,
        scan_id: u32,
        err: crate::Error,
    ) -> crate::Error {
        let counter = match kind {
            PayloadKind::Profile => &self.profile_decode_failures,
            PayloadKind::Centroid => &self.centroid_decode_failures,
        };
        let previous = counter.fetch_add(1, Ordering::Relaxed);
        if previous == 0 {
            log::warn!(
                "{}: {} block for scanId={} failed to decode: {} \
                 (further {} failures in this bundle are logged at debug level)",
                self.bundle_name,
                kind.name(),
                scan_id,
                err,
                kind.name(),
            );
        } else {
            log::debug!(
                "{}: {} block for scanId={} failed to decode: {}",
                self.bundle_name,
                kind.name(),
                scan_id,
                err,
            );
        }
        crate::Error::Scan {
            scan_id,
            block: kind.name(),
            source: Box::new(err),
        }
    }

    /// Decode the peak arrays of scan `scan_idx`. The profile block is
    /// used when present; the centroid block when there is no profile
    /// block or the profile block holds no points. A present block that
    /// cannot be read or decoded is an error; there is no fallback from a
    /// failed profile block to the centroid block.
    fn decode_peaks(
        &self,
        scan_idx: usize,
        rec: &ScanRecord,
    ) -> crate::Result<(Vec<f64>, Vec<f32>, ScanMode)> {
        let calibration = self.calibration(scan_idx);
        if let Some(params) = &rec.profile_params {
            let spec = calibration
                .ok_or_else(|| crate::Error::Parse("profile block has no mass calibration".into()))
                .and_then(|cal| {
                    crate::raw::read_bytes(
                        &self.profile_path,
                        params.offset,
                        params.byte_count as usize,
                    )
                    .and_then(|bytes| decode_profile_block(&bytes, params, cal))
                })
                .map_err(|err| {
                    self.record_decode_failure(PayloadKind::Profile, rec.scan_id, err)
                })?;
            if !spec.mz.is_empty() || rec.centroid_params.is_none() {
                return Ok((spec.mz, spec.intensity, ScanMode::Profile));
            }
        }

        if let Some(params) = &rec.centroid_params {
            let spec =
                crate::raw::read_bytes(&self.peak_path, params.offset, params.byte_count as usize)
                    .and_then(|bytes| decode_peak_block(&bytes, params, calibration))
                    .map_err(|err| {
                        self.record_decode_failure(PayloadKind::Centroid, rec.scan_id, err)
                    })?;
            return Ok((spec.mz, spec.intensity, ScanMode::Centroid));
        }

        Ok((Vec::new(), Vec::new(), ScanMode::Centroid))
    }

    /// Build the spectrum for scan `scan_idx`.
    fn spectrum(
        &self,
        scan_idx: usize,
        rec: &ScanRecord,
        analyzer: Analyzer,
    ) -> crate::Result<SpectrumRecord> {
        let (mz, intensity, scan_mode) = self.decode_peaks(scan_idx, rec)?;

        let mut extra = ::std::collections::BTreeMap::new();
        if let Some(value) = rec.mrm_channel_id {
            extra.insert("openaraw.mrm_channel_id".to_string(), value.to_string());
        }
        for (kind, params) in [
            ("profile", &rec.profile_params),
            ("centroid", &rec.centroid_params),
        ] {
            if let Some(params) = params {
                let prefix = format!("openaraw.{kind}");
                extra.insert(format!("{prefix}.format_id"), params.format_id.to_string());
                extra.insert(format!("{prefix}.offset"), params.offset.to_string());
                extra.insert(
                    format!("{prefix}.byte_count"),
                    params.byte_count.to_string(),
                );
                extra.insert(
                    format!("{prefix}.point_count"),
                    params.point_count.to_string(),
                );
                if let Some(value) = params.uncompressed_byte_count {
                    extra.insert(
                        format!("{prefix}.uncompressed_byte_count"),
                        value.to_string(),
                    );
                }
            }
        }
        if let Some(id) = rec.calibration_id {
            extra.insert("openaraw.calibration_id".to_string(), id.to_string());
        }
        let polarity = rec.polarity();
        if polarity.is_none() {
            // Not 0 (positive) or 1 (negative): keep the raw value rather
            // than guess a polarity.
            extra.insert(
                "openaraw.ion_polarity".to_string(),
                rec.ion_polarity.to_string(),
            );
        }

        let precursor = (rec.ms_level >= 2).then(|| {
            let mut precursor = PrecursorInfo::default()
                // The record holds no m/z field distinct from target_mz
                // (selected_mz stays unset); see
                // docs/format/06-known-limitations.md.
                .with_collision_energy(rec.collision_energy);
            if let Some(mrm_id) = rec.mrm_channel_id {
                precursor = precursor.with_precursor_native_id(format!("mrm_channel={mrm_id}"));
            } else if let Some(mz) = rec.target_mz {
                // Q-TOF MS2: the parent scan is the record's
                // DataDependentScanParamType.DDScanID (per the bundle's
                // MSScan.xsd), which is not decoded, so the precursor
                // reference is left unset.
                precursor = precursor.with_target_mz(mz);
            }
            precursor
        });

        // Not set:
        // - inv_mobility / inv_mobility_per_peak: no ion-mobility (6560
        //   IM-QTOF) acquisition exists in the validation corpus to decode
        //   a drift-time field against. See
        //   docs/format/06-known-limitations.md#3-no-ion-mobility--ccs-support-agilent-6560-im-qtof.
        // - faims_cv: Agilent instruments have no FAIMS interface.
        Ok(
            SpectrumRecord::new(scan_idx, rec.scan_id, format!("scanId={}", rec.scan_id))
                .with_ms_level(rec.ms_level)
                .with_polarity(polarity)
                .with_scan_mode(scan_mode)
                .with_analyzer(analyzer)
                .with_retention_time_sec(rec.retention_time_min * 60.0)
                .with_total_ion_current(rec.tic)
                .with_base_peak_mz(rec.base_peak_mz)
                .with_base_peak_intensity(rec.base_peak_intensity)
                .with_low_mz(rec.min_x)
                .with_high_mz(rec.max_x)
                .with_precursor(precursor)
                .with_peaks(mz, intensity)
                .with_extra_map(extra),
        )
    }
}

impl SpectrumSource for Reader {
    fn run_metadata(&self) -> RunMetadata {
        let mut extra = ::std::collections::BTreeMap::new();
        extra.insert(
            "openaraw.msscan_header_size".to_string(),
            self.msscan.global_header_size.to_string(),
        );
        extra.insert(
            "openaraw.msscan_stride".to_string(),
            self.msscan.stride.to_string(),
        );
        if let Some(device) = &self.device {
            extra.insert("openaraw.device_name".to_string(), device.name.clone());
            extra.insert("openaraw.device_model".to_string(), device.model.clone());
        }
        // PSI-MS: MS:1001509 "Agilent MassHunter format" (is_a MS:1000560
        // mass spectrometer file format) and MS:1001508 "Agilent MassHunter
        // nativeID format" (`scanId=...`).
        //
        // Not set:
        // - acquisition software name / version: `Contents.xml`'s
        //   `AcqSoftwareVersion` element exists in the corpus (see
        //   docs/format/01-msscan.md) but is not extracted.
        // - mobility_array_kind: no ion-mobility instrument (Agilent 6560
        //   IM-QTOF) appears in the validation corpus; see
        //   docs/format/06-known-limitations.md#3-no-ion-mobility--ccs-support-agilent-6560-im-qtof.
        RunMetadata::new(
            self.bundle_name.clone(),
            CvTerm::new("MS:1001509", "Agilent MassHunter format"),
            CvTerm::new("MS:1001508", "Agilent MassHunter nativeID format"),
            self.instrument.clone(),
            "openaraw",
            env!("CARGO_PKG_VERSION"),
        )
        .with_start_timestamp(self.start_timestamp.clone())
        .with_extra_map(extra)
    }

    fn spectrum_count_hint(&self) -> Option<usize> {
        Some(self.msscan.records.len())
    }

    /// Yield one item per `MSScan.bin` record, in file order. A scan whose
    /// payload block cannot be read or decoded (including a Q-TOF scan
    /// without a usable mass calibration) is yielded as an `Err` wrapping
    /// [`crate::Error::Scan`]; iteration continues with the next scan.
    fn iter_spectra<'a>(&'a mut self) -> SpectrumIter<'a> {
        // Resolved once in `Reader::open` from `Devices.xml` (falling back to
        // the record-stride guess only when that's unavailable) - the same
        // analyzer type for every scan in the run, so there's no need to
        // re-derive it per spectrum. See `resolve_analyzer`.
        let analyzer = self.analyzer;
        let this: &'a Reader = self;
        Box::new(
            this.msscan
                .records
                .iter()
                .enumerate()
                .map(move |(scan_idx, rec)| {
                    this.spectrum(scan_idx, rec, analyzer)
                        .map_err(openmassspec_core::Error::from)
                }),
        )
    }

    /// Emit the run's summary chromatograms (TIC and BPC) over the MS1
    /// spectra: each point is the spectrum's instrument-reported `TIC` /
    /// base peak where the record carries them (Q-TOF), otherwise the sum
    /// and maximum of its intensities. Runs without MS1 scans (QQQ MRM)
    /// yield none; per-transition SRM chromatograms are not produced. If
    /// any spectrum fails to decode, the iterator yields that error alone.
    fn iter_chromatograms<'a>(&'a mut self) -> ChromatogramIter<'a> {
        match summary_chromatograms(self.iter_spectra()) {
            Ok(records) => Box::new(records.into_iter().map(Ok)),
            Err(err) => Box::new(std::iter::once(Err(err))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::msscan::ScanRecord;
    use openmassspec_core::Polarity;

    /// Minimal `MSScan` carrying just the field [`resolve_analyzer`] and
    /// [`resolve_instrument`] read: the record stride used for their
    /// Devices.xml-unavailable fallback.
    fn msscan_with_stride(stride: u32) -> MSScan {
        MSScan {
            global_header_size: 0,
            stride,
            records: Vec::new(),
        }
    }

    #[test]
    fn resolve_analyzer_prefers_devices_xml_qtof_over_stride() {
        let device = DeviceInfo {
            name: "QTOF".to_string(),
            model: "G6550A".to_string(),
        };
        // Stride says QQQ, but a real Devices.xml device entry must win.
        let msscan = msscan_with_stride(150);
        assert_eq!(resolve_analyzer(Some(&device), &msscan), Analyzer::TOFMS);
    }

    #[test]
    fn resolve_analyzer_prefers_devices_xml_tandem_quadrupole_over_stride() {
        let device = DeviceInfo {
            name: "TandemQuadrupole".to_string(),
            model: "G6410A".to_string(),
        };
        // Stride says Q-TOF, but a real Devices.xml device entry must win.
        let msscan = msscan_with_stride(284);
        assert_eq!(resolve_analyzer(Some(&device), &msscan), Analyzer::TQMS);
    }

    #[test]
    fn resolve_analyzer_falls_back_to_stride_when_devices_xml_missing() {
        assert_eq!(
            resolve_analyzer(None, &msscan_with_stride(220)),
            Analyzer::TOFMS
        );
        assert_eq!(
            resolve_analyzer(None, &msscan_with_stride(150)),
            Analyzer::TQMS
        );
    }

    /// Without a usable `Devices.xml`, every Q-TOF stride (216, 220, 284)
    /// must resolve to Q-TOF and both QQQ strides (186, 196) to QQQ, for the
    /// analyzer and the instrument term alike.
    #[test]
    fn stride_fallback_classifies_every_known_stride() {
        for (stride, qtof) in [
            (284, true),
            (220, true),
            (216, true),
            (196, false),
            (186, false),
        ] {
            let msscan = msscan_with_stride(stride);
            let (analyzer, name) = if qtof {
                (Analyzer::TOFMS, "Agilent Q-TOF")
            } else {
                (Analyzer::TQMS, "Agilent QQQ")
            };
            assert_eq!(resolve_analyzer(None, &msscan), analyzer, "stride {stride}");
            let instrument = resolve_instrument(None, &msscan);
            assert_eq!(instrument.accession, "MS:1000461", "stride {stride}");
            assert_eq!(instrument.name, name, "stride {stride}");
        }
    }

    #[test]
    fn resolve_analyzer_falls_back_to_stride_for_unrecognized_device_name() {
        // A device name outside the two seen across the validation corpus
        // (see `resolve_analyzer` docs) is treated the same as a missing
        // Devices.xml, rather than guessed at.
        let device = DeviceInfo {
            name: "SomeFutureInstrument".to_string(),
            model: "G9999A".to_string(),
        };
        assert_eq!(
            resolve_analyzer(Some(&device), &msscan_with_stride(220)),
            Analyzer::TOFMS
        );
    }

    /// In-memory `Reader` over synthetic scan records, with no backing
    /// `.d` directory (payload reads fail, which iter_spectra tolerates).
    fn reader_with_records(stride: u32, records: Vec<ScanRecord>) -> Reader {
        Reader {
            dir: PathBuf::new(),
            bundle_name: String::new(),
            msscan: MSScan {
                global_header_size: 0,
                stride,
                records,
            },
            device: None,
            peak_path: PathBuf::new(),
            profile_path: PathBuf::new(),
            instrument: CvTerm::new("MS:1000490", "Agilent instrument model"),
            analyzer: Analyzer::TOFMS,
            start_timestamp: None,
            calibrations: Vec::new(),
            profile_decode_failures: AtomicUsize::new(0),
            centroid_decode_failures: AtomicUsize::new(0),
        }
    }

    #[test]
    fn run_metadata_uses_agilent_masshunter_cv_terms() {
        let meta = reader_with_records(284, Vec::new()).run_metadata();
        assert_eq!(meta.source_file_format.accession, "MS:1001509");
        assert_eq!(meta.source_file_format.name, "Agilent MassHunter format");
        assert_eq!(meta.native_id_format.accession, "MS:1001508");
        assert_eq!(
            meta.native_id_format.name,
            "Agilent MassHunter nativeID format"
        );
    }

    #[test]
    fn qtof_ms2_precursor_does_not_reference_its_own_scan() {
        let record = ScanRecord {
            scan_id: 42,
            retention_time_min: 1.0,
            ms_level: 2,
            min_x: None,
            max_x: None,
            tic: None,
            base_peak_mz: None,
            base_peak_intensity: None,
            calibration_id: None,
            ion_polarity: 0,
            mrm_channel_id: None,
            target_mz: Some(500.25),
            collision_energy: Some(20.0),
            profile_params: None,
            centroid_params: None,
        };
        let mut reader = reader_with_records(284, vec![record]);
        let spectrum = reader.iter_spectra().next().unwrap().unwrap();
        let precursor = spectrum.precursor.expect("MS2 has precursor info");
        assert_eq!(precursor.target_mz, Some(500.25));
        assert_eq!(precursor.precursor_native_id, None);
    }

    #[test]
    fn unreadable_payloads_are_errors() {
        let params = |format_id| crate::raw::msscan::SpectrumParams {
            format_id,
            offset: 0,
            byte_count: 12,
            point_count: 1,
            uncompressed_byte_count: Some(20),
        };
        let record = ScanRecord {
            scan_id: 7,
            retention_time_min: 1.0,
            ms_level: 1,
            min_x: None,
            max_x: None,
            tic: None,
            base_peak_mz: None,
            base_peak_intensity: None,
            calibration_id: None,
            ion_polarity: 0,
            mrm_channel_id: None,
            target_mz: None,
            collision_energy: None,
            profile_params: Some(params(1)),
            centroid_params: Some(params(2)),
        };
        // Empty paths: payload reads fail. A failed profile block is an
        // error; the centroid block is not tried.
        let mut reader = reader_with_records(284, vec![record.clone(), record.clone()]);
        let spectra: Vec<_> = reader.iter_spectra().collect();
        assert_eq!(spectra.len(), 2);
        for item in &spectra {
            let Err(openmassspec_core::Error::Vendor(err)) = item else {
                panic!("expected a vendor error, got {item:?}");
            };
            let err = err.downcast_ref::<crate::Error>().expect("openaraw error");
            assert!(
                matches!(
                    err,
                    crate::Error::Scan {
                        scan_id: 7,
                        block: "profile",
                        ..
                    }
                ),
                "{err:?}"
            );
        }
        assert_eq!(reader.profile_decode_failures(), 2);
        assert_eq!(reader.centroid_decode_failures(), 0);

        // Centroid block alone.
        let mut centroid_only = record;
        centroid_only.profile_params = None;
        let mut reader = reader_with_records(284, vec![centroid_only]);
        assert!(reader.iter_spectra().next().unwrap().is_err());
        assert_eq!(reader.centroid_decode_failures(), 1);
        // The chromatograms report the same error instead of a trace with
        // a missing point.
        let chroms: Vec<_> = reader.iter_chromatograms().collect();
        assert_eq!(chroms.len(), 1);
        assert!(chroms[0].is_err());
    }

    #[test]
    fn spectrum_exposes_instrument_reported_summary_fields() {
        let record = ScanRecord {
            scan_id: 1,
            retention_time_min: 0.5,
            ms_level: 1,
            min_x: Some(100.0),
            max_x: Some(1_000.0),
            tic: Some(2.5e7),
            base_peak_mz: Some(513.9653233122384),
            base_peak_intensity: Some(1_572_319.625),
            calibration_id: Some(1),
            ion_polarity: 0,
            mrm_channel_id: None,
            target_mz: None,
            collision_energy: None,
            profile_params: None,
            centroid_params: None,
        };
        let mut reader = reader_with_records(284, vec![record]);

        let spectrum = reader.iter_spectra().next().unwrap().unwrap();
        assert_eq!(spectrum.base_peak_mz, Some(513.9653233122384));
        assert_eq!(spectrum.base_peak_intensity, Some(1_572_319.625));
        assert_eq!(spectrum.total_ion_current, Some(2.5e7));
        assert_eq!(
            spectrum.extra.get("openaraw.calibration_id"),
            Some(&"1".to_string())
        );
    }

    #[test]
    fn spectrum_extra_is_empty_without_optional_fields() {
        let record = ScanRecord {
            scan_id: 1,
            retention_time_min: 0.5,
            ms_level: 1,
            min_x: None,
            max_x: None,
            tic: None,
            base_peak_mz: None,
            base_peak_intensity: None,
            calibration_id: None,
            ion_polarity: 0,
            mrm_channel_id: None,
            target_mz: None,
            collision_energy: None,
            profile_params: None,
            centroid_params: None,
        };
        let mut reader = reader_with_records(220, vec![record]);

        let spectrum = reader.iter_spectra().next().unwrap().unwrap();
        assert!(spectrum.extra.is_empty());
    }

    #[test]
    fn spectrum_polarity_comes_from_ion_polarity() {
        let record = |scan_id, ion_polarity| ScanRecord {
            scan_id,
            retention_time_min: 0.5,
            ms_level: 1,
            min_x: None,
            max_x: None,
            tic: None,
            base_peak_mz: None,
            base_peak_intensity: None,
            calibration_id: None,
            ion_polarity,
            mrm_channel_id: None,
            target_mz: None,
            collision_energy: None,
            profile_params: None,
            centroid_params: None,
        };
        let mut reader = reader_with_records(220, vec![record(1, 0), record(2, 1), record(3, 5)]);
        let spectra: Vec<_> = reader.iter_spectra().map(Result::unwrap).collect();
        assert_eq!(spectra[0].polarity, Some(Polarity::Positive));
        assert_eq!(spectra[1].polarity, Some(Polarity::Negative));
        assert_eq!(spectra[2].polarity, None);
        assert_eq!(spectra[0].extra.get("openaraw.ion_polarity"), None);
        assert_eq!(spectra[1].extra.get("openaraw.ion_polarity"), None);
        assert_eq!(
            spectra[2].extra.get("openaraw.ion_polarity"),
            Some(&"5".to_string())
        );
    }

    /// Minimal MS-level spectrum carrying just the fields
    /// [`summary_chromatograms`] reads: level, retention time, and the
    /// intensity/mz arrays it aggregates.
    fn spectrum(
        index: usize,
        ms_level: u32,
        rt_sec: f64,
        mz: Vec<f64>,
        intensity: Vec<f32>,
    ) -> openmassspec_core::Result<SpectrumRecord> {
        Ok(
            SpectrumRecord::new(index, index as u32 + 1, format!("scanId={}", index + 1))
                .with_ms_level(ms_level)
                .with_retention_time_sec(rt_sec)
                .with_peaks(mz, intensity),
        )
    }

    #[test]
    fn summary_chromatograms_emit_tic_and_bpc_over_ms1_scans_only() {
        // Two MS1 survey scans with an MS2 scan interleaved. The MS2 scan,
        // and its (here deliberately huge) intensities, must not appear in
        // either trace.
        let spectra = vec![
            spectrum(0, 1, 60.0, vec![100.0, 200.0], vec![10.0, 30.0]),
            spectrum(1, 2, 60.5, vec![150.0], vec![9999.0]),
            spectrum(
                2,
                1,
                120.0,
                vec![100.0, 200.0, 300.0],
                vec![5.0, 50.0, 20.0],
            ),
        ];

        let chroms = summary_chromatograms(spectra.into_iter()).unwrap();
        assert_eq!(chroms.len(), 2, "expected exactly a TIC and a BPC trace");

        let tic = &chroms[0];
        assert_eq!(tic.id, "TIC");
        assert_eq!(tic.index, 0);
        assert_eq!(
            tic.chromatogram_type.as_ref().unwrap().accession,
            "MS:1000235"
        );
        // MS1 retention times only, passed through unchanged (already seconds).
        assert_eq!(tic.time_sec, vec![60.0, 120.0]);
        // TIC = sum of each MS1 scan's intensities.
        assert_eq!(tic.intensity, vec![40.0, 75.0]);

        let bpc = &chroms[1];
        assert_eq!(bpc.id, "BPC");
        assert_eq!(bpc.index, 1);
        assert_eq!(
            bpc.chromatogram_type.as_ref().unwrap().accession,
            "MS:1000628"
        );
        assert_eq!(bpc.time_sec, vec![60.0, 120.0]);
        // BPC = the most intense point in each MS1 scan.
        assert_eq!(bpc.intensity, vec![30.0, 50.0]);
    }

    #[test]
    fn summary_chromatograms_empty_when_no_ms1_scans() {
        // QQQ MRM acquisitions have no MS1 scans, so no survey trace can be
        // built - we emit nothing rather than an empty-typed chromatogram.
        let spectra = vec![
            spectrum(0, 2, 60.0, vec![150.0], vec![100.0]),
            spectrum(1, 2, 60.0, vec![250.0], vec![200.0]),
        ];
        assert!(summary_chromatograms(spectra.into_iter())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn summary_chromatograms_handle_empty_spectra() {
        // A decoded-but-empty MS1 scan contributes a zero point to both
        // traces rather than being dropped, keeping the time axes aligned.
        let spectra = vec![spectrum(0, 1, 30.0, vec![], vec![])];
        let chroms = summary_chromatograms(spectra.into_iter()).unwrap();
        assert_eq!(chroms.len(), 2);
        assert_eq!(chroms[0].intensity, vec![0.0]);
        assert_eq!(chroms[1].intensity, vec![0.0]);
    }

    /// Which calibration files [`synthetic_qtof_bundle`] writes. Both hold
    /// the same `Traditional`-only calibration.
    struct CalFiles {
        masscal: bool,
        default_xml: bool,
    }

    /// Write a minimal stride-284 Q-TOF bundle (`CalibrationID` 1) with one
    /// scan carrying a profile block and a centroid block, plus the
    /// calibration files selected by `files`. Returns the bundle directory.
    fn synthetic_qtof_bundle(name: &str, files: CalFiles) -> PathBuf {
        use byteorder::{ByteOrder, LittleEndian};
        let dir = std::env::temp_dir().join(format!(
            "openaraw-reader-test-{}-{name}.d",
            std::process::id()
        ));
        let acq = dir.join("AcqData");
        std::fs::create_dir_all(&acq).unwrap();
        let encode = |data: &[u8]| -> Vec<u8> {
            let mut out = Vec::new();
            for chunk in data.chunks(32) {
                out.push((chunk.len() - 1) as u8);
                out.extend_from_slice(chunk);
            }
            out
        };
        let ref_cal = TofCalibration::traditional_only(3.47994641e-4, 12.5);

        // Centroid: two flight times, then two intensities.
        let mut peak = vec![0u8; 68];
        for mz in [121.050873f64, 922.009798] {
            peak.extend_from_slice(&ref_cal.flight_time(mz).to_le_bytes());
        }
        peak.extend_from_slice(&5.0f32.to_le_bytes());
        peak.extend_from_slice(&7.0f32.to_le_bytes());
        std::fs::write(acq.join("MSPeak.bin"), &peak).unwrap();

        // Profile: x0, dx, three int32 intensities.
        let mut raw = Vec::new();
        raw.extend_from_slice(&ref_cal.flight_time(500.0).to_le_bytes());
        raw.extend_from_slice(&0.5f64.to_le_bytes());
        for v in [1i32, 9, 2] {
            raw.extend_from_slice(&v.to_le_bytes());
        }
        let mut profile = vec![0u8; 68];
        let compressed = encode(&raw);
        profile.extend_from_slice(&compressed);
        std::fs::write(acq.join("MSProfile.bin"), &profile).unwrap();

        let mut scan = vec![0u8; 92];
        LittleEndian::write_u32(&mut scan[0..4], 257);
        LittleEndian::write_u32(&mut scan[88..92], 92);
        let mut r = vec![0u8; 284];
        LittleEndian::write_u32(&mut r[0..4], 1);
        LittleEndian::write_u16(&mut r[20..22], 1);
        LittleEndian::write_f64(&mut r[28..36], 12.0);
        LittleEndian::write_f64(&mut r[36..44], 922.009798);
        LittleEndian::write_f64(&mut r[44..52], 7.0);
        LittleEndian::write_i32(&mut r[52..56], 1);
        LittleEndian::write_u32(&mut r[156..160], 1);
        LittleEndian::write_u64(&mut r[160..168], 68);
        LittleEndian::write_u32(&mut r[168..172], compressed.len() as u32);
        LittleEndian::write_u32(&mut r[172..176], 3);
        LittleEndian::write_u32(&mut r[176..180], raw.len() as u32);
        LittleEndian::write_u32(&mut r[220..224], 2);
        LittleEndian::write_u64(&mut r[224..232], 68);
        LittleEndian::write_u32(&mut r[232..236], 24);
        LittleEndian::write_u32(&mut r[236..240], 2);
        scan.extend(r);
        std::fs::write(acq.join("MSScan.bin"), &scan).unwrap();

        if files.masscal {
            let mut bytes = vec![0u8; 72];
            let mut rec = vec![0u8; 84];
            LittleEndian::write_u32(&mut rec[0..4], 10);
            LittleEndian::write_f64(&mut rec[4..12], ref_cal.a);
            LittleEndian::write_f64(&mut rec[12..20], ref_cal.t0);
            bytes.extend(rec);
            std::fs::write(acq.join("MSMassCal.bin"), &bytes).unwrap();
        }
        if files.default_xml {
            let mut poly = String::new();
            for n in 1..=8 {
                poly.push_str(&format!("<Value Number=\"{n}\">0</Value>"));
            }
            let xml = format!(
                "<DefaultMassCalibration><DefaultCalibrations>\
                 <DefaultCalibration DefaultCalibrationID=\"1\">\
                 <Step Number=\"1\"><CalibrationFormula>Traditional</CalibrationFormula>\
                 <NumberOfCoefficients>2</NumberOfCoefficients><ValueUseFlags>0</ValueUseFlags>\
                 <Values><Value Number=\"1\">{:E}</Value><Value Number=\"2\">{:E}</Value></Values></Step>\
                 <Step Number=\"2\"><CalibrationFormula>Polynomial</CalibrationFormula>\
                 <NumberOfCoefficients>8</NumberOfCoefficients><ValueUseFlags>0</ValueUseFlags>\
                 <Values>{poly}</Values></Step>\
                 </DefaultCalibration></DefaultCalibrations></DefaultMassCalibration>",
                ref_cal.a, ref_cal.t0
            );
            std::fs::write(acq.join("DefaultMassCal.xml"), xml).unwrap();
        }
        dir
    }

    fn assert_calibrated(reader: &mut Reader) {
        let spectrum = reader.iter_spectra().next().unwrap().unwrap();
        assert_eq!(spectrum.scan_mode, Some(ScanMode::Profile));
        assert_eq!(spectrum.intensity, vec![1.0, 9.0, 2.0]);
        assert!((spectrum.mz[0] - 500.0).abs() < 1e-9);
        assert!(spectrum.mz.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(spectrum.total_ion_current, Some(12.0));
        assert_eq!(spectrum.base_peak_mz, Some(922.009798));
        assert_eq!(reader.profile_decode_failures(), 0);

        // A profile block with no points: the centroid block is used and
        // the spectrum is labelled centroid.
        if let Some(params) = reader.msscan.records[0].profile_params.as_mut() {
            params.point_count = 0;
        }
        let spectrum = reader.iter_spectra().next().unwrap().unwrap();
        assert_eq!(spectrum.scan_mode, Some(ScanMode::Centroid));
        assert_eq!(spectrum.mz.len(), 2);

        // Centroid path, exercised by dropping the profile block.
        reader.msscan.records[0].profile_params = None;
        let spectrum = reader.iter_spectra().next().unwrap().unwrap();
        assert_eq!(spectrum.scan_mode, Some(ScanMode::Centroid));
        assert!((spectrum.mz[0] - 121.050873).abs() < 1e-9);
        assert!((spectrum.mz[1] - 922.009798).abs() < 1e-9);
        assert_eq!(reader.centroid_decode_failures(), 0);
    }

    fn assert_uncalibrated(reader: &mut Reader) {
        assert_eq!(reader.calibration(0), None);
        assert!(reader.iter_spectra().next().unwrap().is_err());
        assert_eq!(reader.profile_decode_failures(), 1);

        // Centroid path, exercised by dropping the profile block.
        reader.msscan.records[0].profile_params = None;
        assert!(reader.iter_spectra().next().unwrap().is_err());
        assert_eq!(reader.centroid_decode_failures(), 1);
    }

    #[test]
    fn qtof_uses_msmasscal_record_with_flags_from_default_xml() {
        let files = CalFiles {
            masscal: true,
            default_xml: true,
        };
        let dir = synthetic_qtof_bundle("masscal", files);
        let mut reader = Reader::open(&dir).unwrap();
        let cal = reader.calibration(0).unwrap();
        assert_eq!(cal.source, CalibrationSource::MassCalRecord);
        assert_calibrated(&mut reader);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn qtof_falls_back_to_default_xml_without_msmasscal() {
        let files = CalFiles {
            masscal: false,
            default_xml: true,
        };
        let dir = synthetic_qtof_bundle("xml-only", files);
        let mut reader = Reader::open(&dir).unwrap();
        let cal = reader.calibration(0).unwrap();
        assert_eq!(cal.source, CalibrationSource::DefaultMassCal);
        assert_calibrated(&mut reader);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn qtof_record_with_zero_polynomial_needs_no_xml() {
        let files = CalFiles {
            masscal: true,
            default_xml: false,
        };
        let dir = synthetic_qtof_bundle("masscal-only", files);
        let mut reader = Reader::open(&dir).unwrap();
        assert_eq!(
            reader.calibration(0).unwrap().source,
            CalibrationSource::MassCalRecord
        );
        assert_calibrated(&mut reader);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn qtof_without_calibration_is_an_error_not_flight_times() {
        let files = CalFiles {
            masscal: false,
            default_xml: false,
        };
        let dir = synthetic_qtof_bundle("uncalibrated", files);
        let mut reader = Reader::open(&dir).unwrap();
        assert_uncalibrated(&mut reader);
        std::fs::remove_dir_all(&dir).ok();
    }
}
