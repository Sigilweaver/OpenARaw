use openaraw::reader::Reader;
use openmassspec_core::conformance::assert_source_invariants;
use std::path::PathBuf;

/// Corpus files live out of tree (they are large real-world acquisitions,
/// not checked into the repo), so these tests skip cleanly when none of a
/// fixture's candidate locations exist, instead of failing the build.
/// Candidates are checked in order and the first one present wins.
fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

fn qtof_fixture() -> Option<PathBuf> {
    first_existing(&[
        // CI / repo-root corpus dir (gitignored; populated by ci.yml's
        // `build` job before `cargo test` runs - see
        // Sigilweaver/OpenARaw#28). This is the one CI actually uses.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/180814-Sample19.d"),
        // Local dev setup with the full research corpus mounted - see
        // CONTRIBUTING.md. Kept as a fallback even though it's ~247 MB
        // zipped and not used by CI.
        PathBuf::from("/workspaces/Projects/Data/ARaw/PXD004426/20140806_TgAAL.d"),
    ])
}

fn qqq_fixture() -> Option<PathBuf> {
    first_existing(&[
        // CI / repo-root corpus dir (gitignored; populated by ci.yml's
        // `build` job before `cargo test` runs - see
        // Sigilweaver/OpenARaw#20). This is the one CI actually uses.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/Cdc19_ubp2_AQUA.d"),
        // Local dev setup with the full research corpus mounted - see
        // CONTRIBUTING.md.
        PathBuf::from("/workspaces/Projects/Data/ARaw/PXD004747/Cdc19_ubp2_AQUA.d"),
    ])
}

#[test]
fn test_qtof_conformance() {
    let Some(path) = qtof_fixture() else {
        eprintln!("skip: no QTOF corpus fixture available");
        return;
    };
    let mut reader = Reader::open(&path).expect("Failed to open QTOF bundle");
    let n = assert_source_invariants(&mut reader).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}

#[test]
fn test_qqq_conformance() {
    let Some(path) = qqq_fixture() else {
        eprintln!("skip: no QQQ corpus fixture available");
        return;
    };
    let mut reader = Reader::open(&path).expect("Failed to open QQQ bundle");
    let n = assert_source_invariants(&mut reader).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}
