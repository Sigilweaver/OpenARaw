//! Print Q-TOF calibration metrics for one or more `.d` bundles: ppm of the
//! decoded base peak against the stored `BasePeakMZ` (origin/main `x/100`,
//! `Traditional` step only, the reader's output, and the full formula with
//! the `DefaultMassCal.xml` calibration), summed intensity against the
//! stored `TIC`, and reference-mass ions. See
//! `tests/support/calibration_metrics.rs`.
//!
//! ```sh
//! cargo run --release --example qtof_calibration_check -- [--no-profile] [--tsv] run.d...
//! ```
//!
//! `--tsv` prints one tab-separated line per bundle instead of a report.

use openaraw::Reader;

#[path = "../tests/support/calibration_metrics.rs"]
mod calibration_metrics;

use calibration_metrics::{Report, Summary};

fn main() {
    stderr_logger_init();
    let mut profile = true;
    let mut tsv = false;
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--no-profile" => profile = false,
            "--tsv" => tsv = true,
            _ => paths.push(arg),
        }
    }
    if paths.is_empty() {
        eprintln!("usage: qtof_calibration_check [--no-profile] [--tsv] <bundle.d>...");
        std::process::exit(2);
    }
    if tsv {
        println!("{}", tsv_header());
    }
    let mut failed = false;
    for path in paths {
        match Reader::open(&path) {
            Ok(reader) => {
                let report = calibration_metrics::evaluate(&reader, profile);
                if tsv {
                    println!("{path}\t{}", tsv_row(&report));
                } else {
                    println!("{path}\n{report}");
                }
            }
            Err(err) => {
                eprintln!("{path}: {err}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn tsv_header() -> String {
    let mut cols = vec![
        "bundle".to_string(),
        "scans".into(),
        "record_cal".into(),
        "default_cal".into(),
        "uncalibrated".into(),
        "centroid_failures".into(),
        "profile_failures".into(),
    ];
    for name in [
        "before", "trad", "after", "formula", "profile", "pformula", "ctic", "ptic", "papex",
    ] {
        for stat in ["n", "med", "med_abs", "p95_abs"] {
            cols.push(format!("{name}_{stat}"));
        }
    }
    cols.push("reference".into());
    cols.join("\t")
}

fn tsv_row(r: &Report) -> String {
    let mut cols = vec![
        r.scans.to_string(),
        r.record_calibrated.to_string(),
        r.default_calibrated.to_string(),
        r.uncalibrated_scans.to_string(),
        r.centroid_failures.to_string(),
        r.profile_failures.to_string(),
    ];
    let summaries: [&Summary; 9] = [
        &r.before_bp_ppm,
        &r.traditional_bp_ppm,
        &r.after_bp_ppm,
        &r.formula_bp_ppm,
        &r.profile_bp_ppm,
        &r.profile_formula_bp_ppm,
        &r.centroid_tic_ratio,
        &r.profile_tic_ratio,
        &r.profile_apex_ratio,
    ];
    for s in summaries {
        cols.push(s.n.to_string());
        for v in [s.median, s.median_abs, s.p95_abs] {
            cols.push(format!("{v:.6}"));
        }
    }
    let refs: Vec<String> = r
        .reference_ions
        .iter()
        .map(|(_, mz, fraction, s)| {
            format!(
                "{mz:.4}:{:.0}%:med={:.3}:p95={:.3}",
                fraction * 100.0,
                s.median,
                s.p95_abs
            )
        })
        .collect();
    cols.push(refs.join(","));
    cols.join("\t")
}

/// Route `log` warnings (calibration load problems) to stderr without
/// adding a logger dependency.
fn stderr_logger_init() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.level() <= log::Level::Info
        }
        fn log(&self, r: &log::Record) {
            if self.enabled(r.metadata()) {
                eprintln!("{}: {}", r.level(), r.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info));
}
