//! Errors

use log::error;
use std::fmt::Debug;
use std::io;
use thiserror::Error;

/// Errors
#[derive(Error, Debug)]
pub enum Error {
    /// Error in HTTP communication
    #[error("Http Error")]
    HttpError(#[from] reqwest::Error),
    /// IO Error
    #[error("IO Error")]
    IOError(#[from] io::Error),
    /// Error acquiring internal lock
    #[error("Lock Error")]
    LockError(String),
    /// Adapter has been shutdown and can no longer be operated on
    #[error("Adapter is shut down")]
    AdapterShutdownError,
    /// Internal channel communication error
    #[error("Channel communication error: `{0}`")]
    ChannelError(String),
}

/// Log error to stderr and at error level
pub fn log_error<E: std::fmt::Display>(e: E) {
    error!("Unexpected error: {}", e);
    eprintln!("Unexpected error: {}", e);
}
