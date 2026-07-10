use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use openaraw::reader::Reader;
use openmassspec_core::conformance::assert_source_invariants;

fn main() {
    let index_path = PathBuf::from("/workspaces/Projects/Data/ARaw/index.csv");
    let base_dir = PathBuf::from("/workspaces/Projects/Data/ARaw");

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
