//! Error handling.

use crate::graphics::rendering::RenderingError;
use std::{error::Error, fmt};

/// Common error enum for the Vortek library.
#[derive(Debug)]
pub enum VortekError {
    RenderingError(RenderingError),
}

pub type VortekResult<T> = Result<T, VortekError>;

impl fmt::Display for VortekError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            VortekError::RenderingError(ref error) => write!(f, "{}", error.message()),
        }
    }
}

impl Error for VortekError {}
