use std::path::PathBuf;
use openaraw::reader::Reader;
use openproteo_core::SpectrumSource;

#[test]
fn test_qtof_conformance() {
    let path = PathBuf::from("/workspaces/Projects/Data/ARaw/PXD004426/20140806_TgAAL.d");

    let mut reader = Reader::open(&path).expect("Failed to open QTOF bundle");
    let spectra: Vec<_> = reader.iter_spectra().collect();

    assert!(!spectra.is_empty(), "QTOF bundle should have spectra");
    let first = &spectra[0];
    
    assert!(first.mz.len() > 0, "Spectrum should have peaks");
    assert_eq!(first.mz.len(), first.intensity.len());
}

#[test]
fn test_qqq_conformance() {
    let path = PathBuf::from("/workspaces/Projects/Data/ARaw/PXD004747/Cdc19_ubp2_AQUA.d");

    let mut reader = Reader::open(&path).expect("Failed to open QQQ bundle");
    let spectra: Vec<_> = reader.iter_spectra().collect();

    assert!(!spectra.is_empty(), "QQQ bundle should have spectra");
    let first = &spectra[0];
    
    assert!(first.mz.len() > 0, "Spectrum should have peaks");
    assert_eq!(first.mz.len(), first.intensity.len());
}
