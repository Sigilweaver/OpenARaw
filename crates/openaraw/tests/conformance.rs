use openaraw::reader::Reader;
use openmassspec_core::conformance::assert_source_invariants;
use std::path::{Path, PathBuf};

/// Corpus files live out of tree (they are large real-world acquisitions,
/// not checked into the repo), so these tests skip cleanly when the corpus
/// is absent - e.g. on CI runners - instead of failing the build.
fn skip_if_absent(path: &Path) -> bool {
    if !path.exists() {
        eprintln!("skip: corpus not present at {}", path.display());
        return true;
    }
    false
}

#[test]
fn test_qtof_conformance() {
    let path = PathBuf::from("/workspaces/Projects/Data/ARaw/PXD004426/20140806_TgAAL.d");
    if skip_if_absent(&path) {
        return;
    }
    let mut reader = Reader::open(&path).expect("Failed to open QTOF bundle");
    let n = assert_source_invariants(&mut reader).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}

#[test]
fn test_qqq_conformance() {
    let path = PathBuf::from("/workspaces/Projects/Data/ARaw/PXD004747/Cdc19_ubp2_AQUA.d");
    if skip_if_absent(&path) {
        return;
    }
    let mut reader = Reader::open(&path).expect("Failed to open QQQ bundle");
    let n = assert_source_invariants(&mut reader).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}
