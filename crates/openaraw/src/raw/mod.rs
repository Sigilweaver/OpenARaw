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
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; length];
    f.read_exact(&mut buf)?;
    Ok(buf)
}
