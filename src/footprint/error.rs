use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unable to inspect {}: {source}", path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("allocated storage measurement is unsupported on this platform")]
    UnsupportedPlatform,

    #[error("invalid footprint root identifier {0}")]
    InvalidRoot(usize),

    #[error("footprint byte total overflow")]
    Overflow,
}

impl Error {
    pub(super) fn inspect(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Inspect { path: path.into(), source }
    }
}
