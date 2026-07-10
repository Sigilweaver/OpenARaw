use openaraw::reader::Reader;
use openmassspec_core::conformance::assert_source_invariants;
use std::path::PathBuf;

#[test]
fn test_qtof_conformance() {
    let path = PathBuf::from("/workspaces/Projects/Data/ARaw/PXD004426/20140806_TgAAL.d");

    let mut reader = Reader::open(&path).expect("Failed to open QTOF bundle");
    let n = assert_source_invariants(&mut reader).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}

#[test]
fn test_qqq_conformance() {
    let path = PathBuf::from("/workspaces/Projects/Data/ARaw/PXD004747/Cdc19_ubp2_AQUA.d");

    let mut reader = Reader::open(&path).expect("Failed to open QQQ bundle");
    let n = assert_source_invariants(&mut reader).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}
