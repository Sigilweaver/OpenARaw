use openaraw::reader::Reader;
use openmassspec_core::{write_mzml_with_options, Check, Leniency, WriteOptions};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "Usage: to_mzml <input.d> <output.mzML> [--allow CHECK]...\n\
     CHECK is a Core check name, for example unknown_collision_energy_unit";

fn main() -> ExitCode {
    let mut positional = Vec::new();
    let mut leniency = Leniency::strict();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--allow" {
            let Some(name) = args.next() else {
                eprintln!("--allow needs a check name\n{USAGE}");
                return ExitCode::from(2);
            };
            match name.parse::<Check>() {
                Ok(check) => leniency = leniency.with(check),
                Err(err) => {
                    eprintln!("{err}\n{USAGE}");
                    return ExitCode::from(2);
                }
            }
        } else {
            positional.push(arg);
        }
    }
    let [input, output] = positional.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    let mut reader = match Reader::open(&input_path) {
        Ok(reader) => reader,
        Err(err) => {
            eprintln!("Failed to open MassHunter .d bundle: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut out_file = match std::fs::File::create(&output_path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Failed to create {}: {err}", output_path.display());
            return ExitCode::FAILURE;
        }
    };

    let options = WriteOptions::new().with_leniency(leniency);
    match write_mzml_with_options(&mut reader, &mut out_file, &options) {
        Ok(summary) => {
            println!(
                "Wrote {} spectra to {}",
                summary.spectra_written,
                output_path.display()
            );
            for (code, count) in &summary.warning_counts {
                println!("warning {code}: {count} records");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("Failed to write mzML: {err}");
            ExitCode::FAILURE
        }
    }
}
