//! Print a digest of everything the reader produces for each bundle: run
//! metadata, every spectrum (all fields, m/z and intensity bits) and the
//! decode-failure counters. Two builds whose digests match produce the same
//! output for that bundle. Used to check that a decoder change leaves other
//! bundles untouched.
//!
//! ```sh
//! cargo run --release --example spectra_digest -- run.d...
//! ```
//!
//! Prints one tab-separated line per bundle: path, spectra, points,
//! profile failures, centroid failures, digest. The digest is a 64-bit
//! FNV-1a variant over the `Debug` text of the metadata and each record
//! (arrays excluded) and the little-endian bytes of the arrays; compare
//! digests from builds with the same toolchain and dependencies only.

use openaraw::Reader;
use openmassspec_core::SpectrumSource;

struct Digest(u64);

impl Digest {
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Digest(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, data: &[u8]) {
        let (words, rest) = data.as_chunks::<8>();
        for word in words {
            self.word(u64::from_le_bytes(*word));
        }
        for &b in rest {
            self.word(u64::from(b));
        }
        self.word(data.len() as u64);
    }

    fn word(&mut self, w: u64) {
        self.0 = (self.0 ^ w).wrapping_mul(Self::PRIME);
    }
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: spectra_digest <bundle.d>...");
        std::process::exit(2);
    }
    let mut failed = false;
    for path in paths {
        let mut reader = match Reader::open(&path) {
            Ok(reader) => reader,
            Err(err) => {
                println!("{path}\topen error: {err}");
                failed = true;
                continue;
            }
        };
        let mut digest = Digest::new();
        digest.bytes(format!("{:?}", reader.run_metadata()).as_bytes());
        let mut spectra = 0usize;
        let mut points = 0usize;
        for mut spectrum in reader.iter_spectra() {
            spectra += 1;
            points += spectrum.mz.len();
            let mz = std::mem::take(&mut spectrum.mz);
            let intensity = std::mem::take(&mut spectrum.intensity);
            digest.bytes(format!("{spectrum:?}").as_bytes());
            let mz_bytes: Vec<u8> = mz.iter().flat_map(|v| v.to_le_bytes()).collect();
            digest.bytes(&mz_bytes);
            let intensity_bytes: Vec<u8> = intensity.iter().flat_map(|v| v.to_le_bytes()).collect();
            digest.bytes(&intensity_bytes);
        }
        let profile_failures = reader.profile_decode_failures();
        let centroid_failures = reader.centroid_decode_failures();
        digest.word(profile_failures as u64);
        digest.word(centroid_failures as u64);
        println!(
            "{path}\t{spectra}\t{points}\t{profile_failures}\t{centroid_failures}\t{:016x}",
            digest.0
        );
    }
    if failed {
        std::process::exit(1);
    }
}
