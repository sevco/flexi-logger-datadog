use log::error;
use std::fmt::Debug;
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Http Error")]
    HttpError(#[from] reqwest::Error),
    #[error("IO Error")]
    IOError(#[from] io::Error),
    #[error("Lock Error")]
    LockError(String),
    #[error("Adapter is shut down")]
    AdapterShutdownError,
}

pub fn log_error<E: std::fmt::Display>(e: E) {
    error!("Unexpected error: {}", e);
    eprintln!("Unexpected error: {}", e);
}
