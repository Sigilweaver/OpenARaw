#![cfg_attr(not(test), warn(clippy::unwrap_used, clippy::expect_used))]

pub mod raw;
pub mod reader;

pub use reader::Reader;

/// Errors returned by OpenARaw.
///
/// The enum is `#[non_exhaustive]`: match it with a wildcard arm.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
    /// A scan's spectrum payload block could not be read or decoded.
    #[error("scanId={scan_id}: {block} block: {source}")]
    #[non_exhaustive]
    Scan {
        /// `ScanID` of the `MSScan.bin` record.
        scan_id: u32,
        /// Which payload block failed: `"profile"` (`MSProfile.bin`) or
        /// `"centroid"` (`MSPeak.bin`).
        block: &'static str,
        /// The underlying read or decode error.
        #[source]
        source: Box<Error>,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for openmassspec_core::Error {
    /// Wrap the error in [`openmassspec_core::Error::Vendor`], keeping
    /// [`Error::Io`] as [`openmassspec_core::Error::Io`].
    fn from(err: Error) -> Self {
        match err {
            Error::Io(io) => openmassspec_core::Error::Io(io),
            other => openmassspec_core::Error::vendor(other),
        }
    }
}
