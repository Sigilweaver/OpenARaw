pub mod lzf;
pub mod metadata;
pub mod msmasscal;
pub mod mspeak;
pub mod msperiodicactuals;
pub mod msprofile;
pub mod msscan;

pub fn read_bytes(path: &std::path::Path, offset: u64, length: usize) -> crate::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let file_len = f.metadata()?.len();

    // `offset`/`length` come from a MSScan.bin record (file-controlled);
    // reject a request that can't possibly be satisfied by this file
    // before allocating `length` bytes for it.
    let remaining = file_len.saturating_sub(offset);
    if length as u64 > remaining {
        return Err(crate::Error::Parse(format!(
            "read_bytes: requested {} bytes at offset {} but {} has only {} bytes remaining",
            length,
            offset,
            path.display(),
            remaining
        )));
    }

    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; length];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_file(contents: &[u8]) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "openaraw-read-bytes-test-{}-{n}.bin",
            std::process::id()
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn reads_requested_slice() {
        let path = temp_file(b"hello world");
        let bytes = read_bytes(&path, 6, 5).unwrap();
        assert_eq!(bytes, b"world");
        std::fs::remove_file(&path).ok();
    }

    /// Regression test: a crafted MSScan.bin record can claim an arbitrary
    /// `ByteCount`/`UncompressedByteCount`. Before the file-size cap, this
    /// would allocate a multi-GB `Vec` before `read_exact` ever ran.
    #[test]
    fn rejects_length_exceeding_remaining_file_size() {
        let path = temp_file(b"short");
        let result = read_bytes(&path, 0, 10 * 1024 * 1024 * 1024);
        assert!(result.is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_offset_past_end_of_file() {
        let path = temp_file(b"short");
        let result = read_bytes(&path, 1000, 1);
        assert!(result.is_err());
        std::fs::remove_file(&path).ok();
    }
}
