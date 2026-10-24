use openaraw::reader::Reader;
use openmassspec_core::conformance::assert_source_invariants;
use std::path::PathBuf;

mod support;
use support::fixture;

fn qtof_fixture() -> Option<PathBuf> {
    // The research-corpus fallback is ~247 MB zipped and not used by CI.
    fixture(
        "QTOF",
        Some("180814-Sample19.d"),
        "PXD004426/20140806_TgAAL.d",
    )
}

fn qqq_fixture() -> Option<PathBuf> {
    fixture(
        "QQQ",
        Some("Cdc19_ubp2_AQUA.d"),
        "PXD004747/Cdc19_ubp2_AQUA.d",
    )
}

#[test]
fn test_qtof_conformance() {
    let Some(path) = qtof_fixture() else {
        return;
    };
    let mut reader = Reader::open(&path).expect("Failed to open QTOF bundle");
    let n = assert_source_invariants(&mut reader).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}

#[test]
fn test_qqq_conformance() {
    let Some(path) = qqq_fixture() else {
        return;
    };
    let mut reader = Reader::open(&path).expect("Failed to open QQQ bundle");
    let n = assert_source_invariants(&mut reader).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}
