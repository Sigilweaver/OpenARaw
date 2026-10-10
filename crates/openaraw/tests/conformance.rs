use openaraw::reader::Reader;
use openmassspec_core::conformance::assert_source_invariants;
use std::path::PathBuf;

/// Corpus files live out of tree (they are large real-world acquisitions,
/// not checked into the repo). Candidates are checked in order and the
/// first one present wins:
///
/// 1. `<repo>/corpus/<bundle>`: populated by ci.yml's `build` job before
///    `cargo test` runs (see Sigilweaver/OpenARaw#20 and #28).
/// 2. `$OPENARAW_CORPUS/<accession>/<bundle>`: a local copy of the full
///    research corpus (see CONTRIBUTING.md).
///
/// When no candidate exists the test skips, unless `REQUIRE_CORPUS=1`
/// is set (as in CI), in which case it fails so a broken fixture download
/// cannot pass silently.
fn fixture(kind: &str, ci_bundle: &str, corpus_relative: &str) -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(ci_bundle)];
    if let Some(root) = std::env::var_os("OPENARAW_CORPUS") {
        candidates.push(PathBuf::from(root).join(corpus_relative));
    }
    if let Some(found) = candidates.iter().find(|p| p.exists()) {
        return Some(found.clone());
    }
    if std::env::var("REQUIRE_CORPUS").as_deref() == Ok("1") {
        panic!("REQUIRE_CORPUS=1 but no {kind} corpus fixture found; checked {candidates:?}");
    }
    eprintln!("skip: no {kind} corpus fixture available (checked {candidates:?})");
    None
}

fn qtof_fixture() -> Option<PathBuf> {
    // The research-corpus fallback is ~247 MB zipped and not used by CI.
    fixture("QTOF", "180814-Sample19.d", "PXD004426/20140806_TgAAL.d")
}

fn qqq_fixture() -> Option<PathBuf> {
    fixture("QQQ", "Cdc19_ubp2_AQUA.d", "PXD004747/Cdc19_ubp2_AQUA.d")
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
