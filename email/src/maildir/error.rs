use std::{any::Any, path::PathBuf, result};
use thiserror::Error;

use crate::{AnyBoxedError, AnyError};

/// The global `Result` alias of the module.
pub type Result<T> = result::Result<T, Error>;

/// The global `Error` enum of the module.
#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot expand path: {0}")]
    ExpandPathFailed(#[from] shellexpand_utils::Error),
    #[error("maildir checkup failed: {0}")]
    CheckingUpMaildirFailed(#[source] maildirpp::Error),
    #[error("cannot create maildir folder structure at {1}")]
    CreateFolderStructureError(#[source] maildirpp::Error, PathBuf),
}

impl AnyError for Error {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl From<Error> for AnyBoxedError {
    fn from(err: Error) -> Self {
        Box::new(err)
    }
}
