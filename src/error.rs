//! Error handling.

use crate::graphics::{rendering::RenderingError, window::WindowError};
use std::{error::Error, fmt};

/// Common error enum for the Vortek library.
#[derive(Debug)]
pub enum VortekError {
    RenderingError(RenderingError),
    WindowError(WindowError),
}

pub type VortekResult<T> = Result<T, VortekError>;

impl fmt::Display for VortekError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            VortekError::RenderingError(ref error) => write!(f, "{}", error.message()),
            VortekError::WindowError(ref error) => write!(f, "{}", error.message()),
        }
    }
}

impl Error for VortekError {}
