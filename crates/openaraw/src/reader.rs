//! High-level reader for an Agilent `.d` bundle directory.

use openmassspec_core::{
    Analyzer, CvTerm, PrecursorInfo, RunMetadata, ScanMode, SpectrumRecord, SpectrumSource,
};
use std::path::{Path, PathBuf};

use crate::raw::metadata::{parse_acquired_time, parse_devices_xml};
use crate::raw::mspeak::{decode_peak_block, PeakSpectrum};
use crate::raw::msprofile::{decode_profile_block, ProfileSpectrum};
use crate::raw::msscan::MSScan;

pub enum DecodedSpectrum {
    Peak(PeakSpectrum),
    Profile(ProfileSpectrum),
}

pub struct Reader {
    pub dir: PathBuf,
    pub bundle_name: String,
    pub msscan: MSScan,
    peak_path: PathBuf,
    profile_path: PathBuf,
    instrument: CvTerm,
    start_timestamp: Option<String>,
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
            return CvTerm::new(acc, *name);
        }
    }
    CvTerm::new("MS:1000490", "Agilent instrument model")
}

/// Resolve the run's instrument CV term, preferring the real device
/// identity parsed from `Devices.xml` over the legacy record-stride
/// guess. Falls back to the stride-based guess (with the same generic
/// `MS:1000461` tag it always used) when `Devices.xml` is missing,
/// unparseable, or lacks the fields we need - this is a real condition
/// in the wild (see `docs/format/06-known-limitations.md`), not just a
/// defensive fallback.
fn resolve_instrument(acq_data: &Path, msscan: &MSScan) -> CvTerm {
    if let Some(device) = parse_devices_xml(&acq_data.join("Devices.xml")) {
        return instrument_cv_for_model(&device.model);
    }

    let is_qtof = msscan.stride >= 220;
    let instrument_name = if is_qtof {
        "Agilent Q-TOF"
    } else {
        "Agilent QQQ"
    };
    CvTerm::new("MS:1000461", instrument_name) // generic agilent node; Devices.xml unavailable
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

        let instrument = resolve_instrument(&acq_data, &msscan);
        let start_timestamp = parse_acquired_time(&acq_data.join("Contents.xml"));

        Ok(Reader {
            dir,
            bundle_name,
            msscan,
            peak_path: acq_data.join("MSPeak.bin"),
            profile_path: acq_data.join("MSProfile.bin"),
            instrument,
            start_timestamp,
        })
    }
}

impl SpectrumSource for Reader {
    fn run_metadata(&self) -> RunMetadata {
        RunMetadata {
            source_file_name: self.bundle_name.clone(),
            source_file_format: CvTerm::new("MS:1002846", "Agilent MassHunter format"),
            native_id_format: CvTerm::new("MS:1002848", "Agilent MassHunter nativeID format"),
            instrument: self.instrument.clone(),
            software_name: "openaraw".to_string(),
            software_version: env!("CARGO_PKG_VERSION").to_string(),
            start_timestamp: self.start_timestamp.clone(),
            mobility_array_kind: None,
        }
    }

    fn spectrum_count_hint(&self) -> Option<usize> {
        Some(self.msscan.records.len())
    }

    fn iter_spectra<'a>(&'a mut self) -> Box<dyn Iterator<Item = SpectrumRecord> + 'a> {
        let mut index = 0;

        let iter = self.msscan.records.clone().into_iter().map(move |rec| {
            let scan_idx = index;
            index += 1;

            let mut mz = vec![];
            let mut intensity = vec![];
            let mut scan_mode = ScanMode::Centroid;

            // Try to read profile data first
            if let Some(params) = &rec.profile_params {
                if let Ok(bytes) = crate::raw::read_bytes(
                    &self.profile_path,
                    params.offset,
                    params.byte_count as usize,
                ) {
                    if let Ok(spec) = decode_profile_block(
                        &bytes,
                        params,
                        rec.min_x.unwrap_or(0.0),
                        rec.max_x.unwrap_or(0.0),
                    ) {
                        mz = spec.mz;
                        intensity = spec.intensity;
                        scan_mode = ScanMode::Profile;
                    }
                }
            }

            // If profile failed or not present, read centroid
            if mz.is_empty() {
                if let Some(params) = &rec.centroid_params {
                    if let Ok(bytes) = crate::raw::read_bytes(
                        &self.peak_path,
                        params.offset,
                        params.byte_count as usize,
                    ) {
                        if let Ok(spec) = decode_peak_block(&bytes, params) {
                            mz = spec.mz;
                            intensity = spec.intensity;
                        }
                    }
                }
            }

            // Create native ID
            let native_id = format!("scanId={}", rec.scan_id);

            // Setup Analyzer type
            let is_qtof = self.msscan.stride >= 220;
            let analyzer = if is_qtof {
                Analyzer::TOFMS
            } else {
                Analyzer::TQMS
            };

            SpectrumRecord {
                index: scan_idx,
                scan_number: rec.scan_id,
                native_id,
                ms_level: rec.ms_level,
                polarity: None, // Could parse from record offset 30 if needed
                scan_mode: Some(scan_mode),
                analyzer: Some(analyzer),
                filter: None,
                retention_time_sec: rec.retention_time_min * 60.0,
                total_ion_current: None,
                base_peak_mz: None,
                base_peak_intensity: None,
                low_mz: rec.min_x,
                high_mz: rec.max_x,
                ion_injection_time_ms: None,
                inv_mobility: None,
                precursor: if rec.ms_level >= 2 {
                    let mut precursor_native_id = None;
                    let mut target_mz = None;
                    if let Some(mrm_id) = rec.mrm_channel_id {
                        precursor_native_id = Some(format!("mrm_channel={}", mrm_id));
                    } else if let Some(mz) = rec.target_mz {
                        target_mz = Some(mz);
                        // For Q-TOF MS2, we can also extract a precursor native ID based on ScanID
                        precursor_native_id = Some(format!("scanId={}", rec.scan_id));
                    }

                    Some(PrecursorInfo {
                        precursor_native_id,
                        target_mz,
                        selected_mz: None,
                        collision_energy: None,
                        ..Default::default()
                    })
                } else {
                    None
                },
                mz,
                intensity,
                inv_mobility_per_peak: None,
            }
        });

        Box::new(iter)
    }
}
