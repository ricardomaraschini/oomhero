use nix;
use std::io;
use std::num;
use thiserror::Error;

// Error enum encapsulates all errors that this libray can return. with it we can easily just
// return a variety of errors without worrying that much about matching and converting.
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Nix(#[from] nix::Error),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    ParseIntError(#[from] num::ParseIntError),
}
