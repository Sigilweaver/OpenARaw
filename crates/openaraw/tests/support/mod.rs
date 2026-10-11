//! Helpers shared by corpus-backed integration tests.

#![allow(dead_code)]

pub mod calibration_metrics;

use std::path::PathBuf;

/// Corpus files live out of tree (they are large real-world acquisitions,
/// not checked into the repo). Candidates are checked in order and the
/// first one present wins:
///
/// 1. `<repo>/corpus/<ci_bundle>`: populated by ci.yml's `build` job before
///    `cargo test` runs (see Sigilweaver/OpenARaw#20 and #28). Pass `None`
///    for bundles CI does not download.
/// 2. `$OPENARAW_CORPUS/<accession>/<bundle>`: a local copy of the full
///    research corpus (see CONTRIBUTING.md).
///
/// When no candidate exists the test skips, unless `REQUIRE_CORPUS=1`
/// is set (as in CI), in which case it fails so a broken fixture download
/// cannot pass silently. Bundles CI does not download (`ci_bundle` is
/// `None`) only fail under `REQUIRE_CORPUS=1` when `OPENARAW_CORPUS` is
/// set, i.e. when the full research corpus is supposed to be present.
pub fn fixture(kind: &str, ci_bundle: Option<&str>, corpus_relative: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(ci_bundle) = ci_bundle {
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus")
                .join(ci_bundle),
        );
    }
    let corpus_root = std::env::var_os("OPENARAW_CORPUS");
    if let Some(root) = &corpus_root {
        candidates.push(PathBuf::from(root).join(corpus_relative));
    }
    if let Some(found) = candidates.iter().find(|p| p.exists()) {
        return Some(found.clone());
    }
    let required = std::env::var("REQUIRE_CORPUS").as_deref() == Ok("1")
        && (ci_bundle.is_some() || corpus_root.is_some());
    if required {
        panic!("REQUIRE_CORPUS=1 but no {kind} corpus fixture found; checked {candidates:?}");
    }
    eprintln!("skip: no {kind} corpus fixture available (checked {candidates:?})");
    None
}
