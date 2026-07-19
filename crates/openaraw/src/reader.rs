//! High-level reader for an Agilent `.d` bundle directory.

use openmassspec_core::{
    Analyzer, ChromatogramRecord, CvTerm, PrecursorInfo, RunMetadata, ScanMode, SpectrumRecord,
    SpectrumSource,
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

/// Build the run-level summary chromatograms (TIC and BPC) from an already
/// decoded spectrum stream.
///
/// This is a pure aggregation of data OpenARaw already decodes - it does no
/// additional binary parsing. Neither `MSScan.bin` nor the payload files
/// expose a confirmed per-scan total-ion-current or base-peak field
/// (`docs/format/01-msscan.md` only tentatively labels the stride-284
/// offset-36 value as "TIC or related intensity metric", so reading it would
/// be a guess), so both traces are derived from the decoded intensity arrays
/// the same way the mzML writer would fill a spectrum's missing summary
/// values: TIC is the sum of intensities in a scan and the base peak is the
/// most intense point, via [`SpectrumRecord::effective_tic`] and
/// [`SpectrumRecord::effective_base_peak`]. Building the traces this way keeps
/// them self-consistent with the spectra actually emitted for the run.
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
fn summary_chromatograms<I>(spectra: I) -> Vec<ChromatogramRecord>
where
    I: Iterator<Item = SpectrumRecord>,
{
    let mut time_sec: Vec<f32> = Vec::new();
    let mut tic: Vec<f32> = Vec::new();
    let mut bpc: Vec<f32> = Vec::new();

    for rec in spectra {
        if rec.ms_level != 1 {
            continue;
        }
        time_sec.push(rec.retention_time_sec as f32);
        tic.push(rec.effective_tic() as f32);
        bpc.push(rec.effective_base_peak().map_or(0.0, |(_, i)| i as f32));
    }

    if time_sec.is_empty() {
        return Vec::new();
    }

    vec![
        ChromatogramRecord {
            index: 0,
            id: "TIC".to_string(),
            chromatogram_type: Some(CvTerm::new("MS:1000235", "total ion current chromatogram")),
            precursor_mz: None,
            product_mz: None,
            time_sec: time_sec.clone(),
            intensity: tic,
        },
        ChromatogramRecord {
            index: 1,
            id: "BPC".to_string(),
            chromatogram_type: Some(CvTerm::new("MS:1000628", "basepeak chromatogram")),
            precursor_mz: None,
            product_mz: None,
            time_sec,
            intensity: bpc,
        },
    ]
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
                // Not recoverable: no byte offset in the ScanRecord was
                // found to correlate with known polarity switches (verified
                // against PXD031771/526b_1.d, whose AcqMethod.xml defines a
                // mid-run positive/negative polarity segment). See
                // docs/format/06-known-limitations.md.
                polarity: None,
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
                faims_cv: None, // Agilent instruments have no FAIMS interface.
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
                        // No second m/z field distinct from target_mz was
                        // found anywhere in the record across the corpus;
                        // see docs/format/06-known-limitations.md.
                        selected_mz: None,
                        collision_energy: rec.collision_energy,
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

    /// Emit the run's summary chromatograms (TIC and BPC), derived from the
    /// decoded MS1 spectra. See [`summary_chromatograms`] for how the traces
    /// are built and why SRM chromatograms are not produced.
    fn iter_chromatograms<'a>(&'a mut self) -> Box<dyn Iterator<Item = ChromatogramRecord> + 'a> {
        let records = summary_chromatograms(self.iter_spectra());
        Box::new(records.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal MS-level spectrum carrying just the fields
    /// [`summary_chromatograms`] reads: level, retention time, and the
    /// intensity/mz arrays it aggregates.
    fn spectrum(
        index: usize,
        ms_level: u32,
        rt_sec: f64,
        mz: Vec<f64>,
        intensity: Vec<f32>,
    ) -> SpectrumRecord {
        SpectrumRecord {
            index,
            scan_number: index as u32 + 1,
            native_id: format!("scanId={}", index + 1),
            ms_level,
            polarity: None,
            scan_mode: None,
            analyzer: None,
            filter: None,
            retention_time_sec: rt_sec,
            total_ion_current: None,
            base_peak_mz: None,
            base_peak_intensity: None,
            low_mz: None,
            high_mz: None,
            ion_injection_time_ms: None,
            inv_mobility: None,
            faims_cv: None,
            precursor: None,
            mz,
            intensity,
            inv_mobility_per_peak: None,
        }
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

        let chroms = summary_chromatograms(spectra.into_iter());
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
        assert!(summary_chromatograms(spectra.into_iter()).is_empty());
    }

    #[test]
    fn summary_chromatograms_handle_empty_spectra() {
        // A decoded-but-empty MS1 scan contributes a zero point to both
        // traces rather than being dropped, keeping the time axes aligned.
        let spectra = vec![spectrum(0, 1, 30.0, vec![], vec![])];
        let chroms = summary_chromatograms(spectra.into_iter());
        assert_eq!(chroms.len(), 2);
        assert_eq!(chroms[0].intensity, vec![0.0]);
        assert_eq!(chroms[1].intensity, vec![0.0]);
    }
}
