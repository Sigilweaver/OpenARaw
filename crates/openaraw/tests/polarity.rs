//! Spectrum polarity from `MSScan.bin`'s `IonPolarity` field, checked
//! against the acquisition method of real bundles.

use openaraw::Reader;
use openmassspec_core::{Polarity, SpectrumSource};

mod support;
use support::fixture;

fn polarities(path: &std::path::Path) -> Vec<(f64, Option<Polarity>)> {
    let mut reader = Reader::open(path).expect("open bundle");
    reader
        .iter_spectra()
        .map(|item| {
            let spectrum = item.expect("spectrum decodes");
            (spectrum.retention_time_sec / 60.0, spectrum.polarity)
        })
        .collect()
}

/// The CI fixtures' methods acquire in positive mode only (every
/// `ionPolarity` entry in `AcqMethod.xml` / `acqmethod.xml` is `Positive`).
#[test]
fn positive_mode_fixtures_are_positive() {
    for (kind, ci_bundle, corpus_relative) in [
        ("QTOF", "180814-Sample19.d", "PXD030293/180814-Sample19.d"),
        ("QQQ", "Cdc19_ubp2_AQUA.d", "PXD004747/Cdc19_ubp2_AQUA.d"),
    ] {
        let Some(path) = fixture(kind, Some(ci_bundle), corpus_relative) else {
            continue;
        };
        let spectra = polarities(&path);
        assert!(!spectra.is_empty());
        assert!(
            spectra
                .iter()
                .all(|(_, polarity)| *polarity == Some(Polarity::Positive)),
            "{kind}: expected every spectrum positive"
        );
    }
}

/// `PXD031771/526b_1.d` switches polarity by time segment: its
/// `AcqMethod.xml` sets the second time segment (from 0.1 min) to
/// `Negative` and the others (from 0, 0.5, 18 and 140 min) to `Positive`.
/// The 38 scans recorded in that segment (TimeSegmentID 2, 0.22 to 0.53
/// min) must be the only negative ones.
#[test]
fn mixed_polarity_run_follows_time_segments() {
    let Some(path) = fixture("mixed-polarity Q-TOF", None, "PXD031771/526b_1.d") else {
        return;
    };
    let spectra = polarities(&path);
    assert!(spectra.iter().all(|(_, polarity)| polarity.is_some()));
    let negative: Vec<usize> = spectra
        .iter()
        .enumerate()
        .filter(|(_, (_, polarity))| *polarity == Some(Polarity::Negative))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(negative, (6..44).collect::<Vec<_>>());
    for (i, (rt_min, _)) in spectra.iter().enumerate() {
        if negative.contains(&i) {
            assert!(
                (0.2..0.6).contains(rt_min),
                "scan index {i} at {rt_min} min"
            );
        }
    }
}
