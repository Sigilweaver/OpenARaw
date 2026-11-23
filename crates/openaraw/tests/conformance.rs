use openaraw::reader::Reader;
use openmassspec_core::conformance::{assert_source_invariants, ConformanceError};
use openmassspec_core::{RunMetadata, SpectrumIter, SpectrumSource};
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

/// The reader with a placeholder `target_mz` on every MS2 spectrum that has
/// no precursor identity. Used only to run the rest of Core's invariants on
/// MRM data, see `test_qqq_conformance`.
struct PlaceholderPrecursors(Reader);

impl SpectrumSource for PlaceholderPrecursors {
    fn run_metadata(&self) -> RunMetadata {
        self.0.run_metadata()
    }

    fn spectrum_count_hint(&self) -> Option<usize> {
        self.0.spectrum_count_hint()
    }

    fn iter_spectra<'a>(&'a mut self) -> SpectrumIter<'a> {
        Box::new(self.0.iter_spectra().map(|item| {
            item.map(|mut spectrum| {
                if let Some(p) = spectrum.precursor.as_mut() {
                    if p.target_mz.is_none() && p.selected_mz.is_none() {
                        p.target_mz = Some(1.0);
                    }
                }
                spectrum
            })
        }))
    }
}

#[test]
fn test_qqq_conformance() {
    let Some(path) = qqq_fixture() else {
        return;
    };
    let mut reader = Reader::open(&path).expect("Failed to open QQQ bundle");
    // An MRM scan names no precursor: the record holds the channel, not
    // the Q1 m/z, and the channel is not a spectrum. Core's harness
    // requires `target_mz`, `selected_mz` or `precursor_native_id` on every
    // MS2 spectrum, so it rejects the first one.
    let err = assert_source_invariants(&mut reader).expect_err("MRM spectra have no precursor id");
    assert!(
        matches!(err, ConformanceError::MissingPrecursor { .. }),
        "{err:?}"
    );
    // Every other invariant holds.
    let mut with_placeholder = PlaceholderPrecursors(reader);
    let n = assert_source_invariants(&mut with_placeholder).expect("conformance");
    assert!(n > 0, "expected at least one spectrum");
}
