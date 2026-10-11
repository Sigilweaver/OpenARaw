//! Q-TOF calibration checked against ground truth stored in the bundle
//! (`MSScan.bin` `BasePeakMZ` / `TIC` / `BasePeakValue`, reference-mass
//! ions). Needs the research corpus (`OPENARAW_CORPUS`) except for the CI
//! fixture; skips otherwise. See `tests/support/calibration_metrics.rs`
//! for what each metric means.
//!
//! Thresholds come from a run of the `qtof_calibration_check` example over
//! all 224 Q-TOF bundles in the corpus (209 decodable, 5.4 million scans;
//! see `docs/format/05-msmasscal.md`, Corpus Evidence):
//!
//! - Formula check (the calibration `BasePeakMZ` was computed with,
//!   against `BasePeakMZ`): worst bundle median |ppm| 0.00012, worst p95
//!   0.00049. The limits (0.001 median, 0.01 p95) leave an 8x / 20x
//!   margin for floating-point and decimal-text rounding. A real formula
//!   error is far larger: `Traditional` step only gives 0.56 ppm median on
//!   the best bundle, an unclamped polynomial about 1 ppm on the affected
//!   scans, a wrong power assignment more than 0.7 ppm.
//! - Summary sums: `TIC` equals the summed intensity of the data the
//!   instrument summarised (centroid or profile) to the printed precision
//!   in every bundle; the limit is 1e-4 relative.
//! - Reference ions: where the instrument applied its per-scan
//!   reference-mass correction, the lock mass lands within 0.01 ppm
//!   (median) of its theoretical m/z (PXD001310 HP-1221: 0.004 ppm). The
//!   0.1 ppm limit fails a reader that ignores the per-scan record (10 ppm
//!   off on PXD001310).

use openaraw::reader::Reader;

mod support;
use support::calibration_metrics::{evaluate, Report, Summary};
use support::fixture;

const FORMULA_MAX_MEDIAN_PPM: f64 = 0.001;
const FORMULA_MAX_P95_PPM: f64 = 0.01;
const TIC_MAX_RELATIVE_ERROR: f64 = 1e-4;
const REFERENCE_MAX_MEDIAN_PPM: f64 = 0.1;

/// Which decoded data the bundle's `MSScan.bin` summary fields describe.
#[derive(Clone, Copy)]
enum Summarised {
    Centroid,
    Profile,
}

fn assert_ppm(name: &str, s: &Summary) {
    assert!(s.n > 0, "{name}: no samples");
    assert!(
        s.median_abs <= FORMULA_MAX_MEDIAN_PPM,
        "{name}: median |ppm| {} > {FORMULA_MAX_MEDIAN_PPM}",
        s.median_abs
    );
    assert!(
        s.p95_abs <= FORMULA_MAX_P95_PPM,
        "{name}: p95 |ppm| {} > {FORMULA_MAX_P95_PPM}",
        s.p95_abs
    );
}

fn assert_ratio_is_one(name: &str, s: &Summary) {
    assert!(s.n > 0, "{name}: no samples");
    assert!(
        (s.median - 1.0).abs() <= TIC_MAX_RELATIVE_ERROR,
        "{name}: median {} != 1",
        s.median
    );
}

/// Open and evaluate a bundle, require every scan to be calibrated and
/// decoded, and check the formula and `TIC` against the data the stored
/// summary describes.
fn check(
    kind: &str,
    ci_bundle: Option<&str>,
    corpus_relative: &str,
    summarised: Summarised,
) -> Option<Report> {
    let path = fixture(kind, ci_bundle, corpus_relative)?;
    let reader = Reader::open(&path).expect("open bundle");
    let report = evaluate(&reader, true);
    eprintln!("{}:\n{report}", path.display());

    assert_eq!(report.uncalibrated_scans, 0, "scans without calibration");
    assert_eq!(report.centroid_failures, 0, "centroid decode failures");
    assert_eq!(report.profile_failures, 0, "profile decode failures");
    match summarised {
        Summarised::Centroid => {
            assert_ppm("centroid formula", &report.formula_bp_ppm);
            assert_ratio_is_one("centroid sum / TIC", &report.centroid_tic_ratio);
        }
        Summarised::Profile => {
            assert_ppm("profile formula", &report.profile_formula_bp_ppm);
            assert_ratio_is_one("profile sum / TIC", &report.profile_tic_ratio);
            assert_ratio_is_one("profile apex / BasePeakValue", &report.profile_apex_ratio);
        }
    }
    Some(report)
}

fn assert_reference_ion(report: &Report, mz: f64) {
    let (_, _, _, s) = report
        .reference_ions
        .iter()
        .find(|(_, m, _, _)| (*m - mz).abs() < 1e-3)
        .unwrap_or_else(|| panic!("reference ion {mz} not found"));
    assert!(
        s.median_abs <= REFERENCE_MAX_MEDIAN_PPM,
        "reference {mz}: median |ppm| {} > {REFERENCE_MAX_MEDIAN_PPM}",
        s.median_abs
    );
}

/// CI fixture: G6550A, centroid only, per-scan records.
#[test]
fn pxd030293_ci_fixture() {
    let Some(report) = check(
        "QTOF",
        Some("180814-Sample19.d"),
        "PXD030293/180814-Sample19.d",
        Summarised::Centroid,
    ) else {
        return;
    };
    assert_eq!(report.record_calibrated, report.scans);
}

/// G6550A, profile + centroid (stride 284), summary from centroids,
/// per-scan records with the reference-mass correction on every scan.
#[test]
fn pxd001310_profile_and_centroid() {
    let Some(report) = check(
        "PXD001310 Q-TOF",
        None,
        "PXD001310/2013-02-26-001 EF CR DMSO 60m r1.d",
        Summarised::Centroid,
    ) else {
        return;
    };
    assert_eq!(report.record_calibrated, report.scans);
    assert!(report.profile_bp_ppm.n > 0, "no profile scans decoded");
    assert_reference_ion(&report, 1221.990637);
}

/// G6530B, centroid only (stride 216), positive and negative segments
/// using `CalibrationID` 1 and 2.
#[test]
fn pxd031771_positive_and_negative_segments() {
    let Some(report) = check(
        "PXD031771 Q-TOF",
        None,
        "PXD031771/526b_1.d",
        Summarised::Centroid,
    ) else {
        return;
    };
    assert_eq!(report.record_calibrated, report.scans);
}

/// G6540B, centroid only, no `MSMassCal.bin`: every scan uses
/// `DefaultMassCal.xml`, so the reader output itself matches `BasePeakMZ`.
#[test]
fn pxd007734_default_xml_fallback() {
    let Some(report) = check(
        "PXD007734 Q-TOF",
        None,
        "PXD007734/160120-S1-band-2-00.d",
        Summarised::Centroid,
    ) else {
        return;
    };
    assert_eq!(report.default_calibrated, report.scans);
    assert_ppm("reader output", &report.after_bp_ppm);
}

/// G6550A, profile only, no `MSMassCal.bin`, summary from profile data.
#[test]
fn pxd038602_profile_only_default_xml() {
    let Some(report) = check(
        "PXD038602 Q-TOF",
        None,
        "PXD038602/Sp_AFP_L3_1219-3_dilx4-1.d",
        Summarised::Profile,
    ) else {
        return;
    };
    assert_eq!(report.default_calibrated, report.scans);
    assert_ppm("reader profile output", &report.profile_bp_ppm);
}

/// G6540A, profile + centroid, no `MSMassCal.bin`, summary from profile
/// data.
#[test]
fn pxd057471_profile_and_centroid_default_xml() {
    let Some(report) = check(
        "PXD057471 Q-TOF",
        None,
        "PXD057471/Band7.d",
        Summarised::Profile,
    ) else {
        return;
    };
    assert_eq!(report.default_calibrated, report.scans);
    assert_ppm("reader profile output", &report.profile_bp_ppm);
}
