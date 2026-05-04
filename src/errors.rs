use metrics_exporter_prometheus;
use nix;
use std::io;
use std::num;
use thiserror::Error;

// Error enum encapsulates all errors that this library can return. With it we can easily just
// return a variety of errors without worrying that much about matching and converting.
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Nix(#[from] nix::Error),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    ParseIntError(#[from] num::ParseIntError),

    #[error(transparent)]
    ParseFloatError(#[from] num::ParseFloatError),

    #[error(transparent)]
    BuildError(#[from] metrics_exporter_prometheus::BuildError),

    #[error("{0}")]
    Message(String),
}
