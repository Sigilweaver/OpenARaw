use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use openaraw::reader::Reader;
use openmassspec_core::conformance::assert_source_invariants;

fn main() {
    // Root of the research corpus (the directory holding index.csv and the
    // per-accession bundle folders). See CONTRIBUTING.md.
    let Some(base_dir) = std::env::var_os("OPENARAW_CORPUS").map(PathBuf::from) else {
        eprintln!("Set OPENARAW_CORPUS to the corpus root (the directory containing index.csv)");
        std::process::exit(2);
    };
    let index_path = base_dir.join("index.csv");

    let file = File::open(&index_path).expect("Failed to open index.csv");
    let reader = BufReader::new(file);

    let mut lines = reader.lines();
    // skip header
    lines.next();

    let mut pass_count = 0;
    let mut fail_count = 0;
    let mut failures = Vec::new();
    let mut total_count = 0;

    for line_result in lines {
        let line = line_result.expect("Failed to read line");
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }

        let d_dir = parts[1];
        let path = base_dir.join(d_dir);

        total_count += 1;
        println!("Scanning {}/{} : {}", total_count, 338, d_dir); // rough estimate of total 338

        match Reader::open(&path) {
            Ok(mut reader) => {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    assert_source_invariants(&mut reader)
                })) {
                    Ok(Ok(_)) => {
                        pass_count += 1;
                    }
                    Ok(Err(e)) => {
                        fail_count += 1;
                        failures.push((d_dir.to_string(), format!("Conformance error: {:?}", e)));
                    }
                    Err(_) => {
                        fail_count += 1;
                        failures.push((
                            d_dir.to_string(),
                            "Panic during conformance check".to_string(),
                        ));
                    }
                }
            }
            Err(e) => {
                fail_count += 1;
                failures.push((d_dir.to_string(), format!("Open/Parse error: {:?}", e)));
            }
        }
    }

    println!("\n=== Scan Summary ===");
    println!("Total scanned: {}", total_count);
    println!("Passed: {}", pass_count);
    println!("Failed: {}", fail_count);

    if fail_count > 0 {
        println!("\nFailures:");
        for (path, reason) in failures {
            println!("- {}: {}", path, reason);
        }
    }
}
