//! High-level reader for an Agilent `.d` bundle directory.

use std::path::{Path, PathBuf};
use openproteo_core::{SpectrumSource, CvTerm, RunMetadata, SpectrumRecord, Analyzer, ScanMode};

use crate::raw::msscan::MSScan;
use crate::raw::mspeak::{decode_peak_block, PeakSpectrum};
use crate::raw::msprofile::{decode_profile_block, ProfileSpectrum};

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

        Ok(Reader {
            dir,
            bundle_name,
            msscan,
            peak_path: acq_data.join("MSPeak.bin"),
            profile_path: acq_data.join("MSProfile.bin"),
        })
    }
}

impl SpectrumSource for Reader {
    fn run_metadata(&self) -> RunMetadata {
        let is_qtof = self.msscan.stride >= 220;
        let instrument_name = if is_qtof { "Agilent Q-TOF" } else { "Agilent QQQ" };

        RunMetadata {
            source_file_name: self.bundle_name.clone(),
            source_file_format: CvTerm::new("MS:1002846", "Agilent MassHunter format"),
            native_id_format: CvTerm::new("MS:1002848", "Agilent MassHunter nativeID format"),
            instrument: CvTerm::new("MS:1000461", instrument_name), // generic agilent node
            software_name: "openaraw".to_string(),
            software_version: env!("CARGO_PKG_VERSION").to_string(),
            start_timestamp: None,
            mobility_array_kind: None,
        }
    }

    fn spectrum_count_hint(&self) -> Option<usize> {
        Some(self.msscan.records.len())
    }

    fn iter_spectra<'a>(&'a mut self) -> Box<dyn Iterator<Item = SpectrumRecord> + 'a> {
        let mut index = 0;
        
        let iter = self.msscan.records.clone().into_iter().filter_map(move |rec| {
            let scan_idx = index;
            index += 1;

            let mut mz = vec![];
            let mut intensity = vec![];
            let mut scan_mode = ScanMode::Centroid;

            // Try to read profile data first
            if let Some(params) = &rec.profile_params {
                if let Ok(bytes) = crate::raw::read_bytes(&self.profile_path, params.offset, params.byte_count as usize) {
                    if let Ok(spec) = decode_profile_block(&bytes, params, rec.min_x.unwrap_or(0.0), rec.max_x.unwrap_or(0.0)) {
                        mz = spec.mz;
                        intensity = spec.intensity;
                        scan_mode = ScanMode::Profile;
                    }
                }
            }
            
            // If profile failed or not present, read centroid
            if mz.is_empty() {
                if let Some(params) = &rec.centroid_params {
                    if let Ok(bytes) = crate::raw::read_bytes(&self.peak_path, params.offset, params.byte_count as usize) {
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
            let analyzer = if is_qtof { Analyzer::TOFMS } else { Analyzer::TQMS };

            Some(SpectrumRecord {
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
                precursor: None,
                mz,
                intensity,
                inv_mobility_per_peak: None,
            })
        });

        Box::new(iter)
    }
}
