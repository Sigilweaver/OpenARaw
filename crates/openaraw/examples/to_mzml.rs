use std::path::PathBuf;
use openaraw::reader::Reader;
use openproteo_core::write_mzml;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: to_mzml <input.d> <output.mzML>");
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let output_path = PathBuf::from(&args[2]);

    let mut reader = Reader::open(&input_path).expect("Failed to open MassHunter .d bundle");
    
    let mut out_file = std::fs::File::create(&output_path).expect("Failed to create output file");
    write_mzml(&mut reader, &mut out_file).expect("Failed to write mzML");
    
    println!("Successfully wrote {}", output_path.display());
}
