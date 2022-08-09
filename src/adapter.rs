//! Writable adapter that manages communication with the async writer task

use crate::error::Error::{AdapterShutdownError, LockError};
use crate::error::{log_error, Error};
use flexi_logger::writers::LogWriter;
use flexi_logger::DeferredNow;
use log::Record;
use std::io;
use std::io::ErrorKind;
use std::sync::Mutex;
use tracing::instrument;

/// Channel for sending log messages
struct LogStream {
    /// Log send channel
    logs: flume::Sender<String>,
}

/// Encapsulation of flush request/response channels
struct FlushStream {
    /// Flush request channel
    request: flume::Sender<()>,
    /// Flush response channel
    response: flume::Receiver<Result<(), Error>>,
}

/// Writable adapter that manages communication with the async writer task
pub struct DataDogAdapter {
    /// Log channel
    log_channel: Mutex<Option<LogStream>>,
    /// Flush channels
    flush_channel: Mutex<Option<FlushStream>>,
}

impl DataDogAdapter {
    /// Create new [`DataDogAdapter`] with channels
    pub fn new(
        logs: flume::Sender<String>,
        flush_request: flume::Sender<()>,
        flush_response: flume::Receiver<Result<(), Error>>,
    ) -> Self {
        Self {
            log_channel: Mutex::new(Some(LogStream { logs })),
            flush_channel: Mutex::new(Some(FlushStream {
                request: flush_request,
                response: flush_response,
            })),
        }
    }
}

impl LogWriter for DataDogAdapter {
    #[instrument(level = "debug", skip_all)]
    fn write(&self, _now: &mut DeferredNow, record: &Record) -> io::Result<()> {
        self.log_channel
            .lock()
            .map_err(|e| {
                io::Error::new(
                    ErrorKind::BrokenPipe,
                    LockError(format!("Failed to acquire logs lock: {}", e)),
                )
            })
            .and_then(|maybe_logs| match &*maybe_logs {
                None => Err(io::Error::new(ErrorKind::BrokenPipe, AdapterShutdownError)),
                Some(stream) => {
                    let log = format!(
                        "{} [{}] {}",
                        record.level(),
                        record.module_path().unwrap_or_default(),
                        record.args()
                    );
                    stream
                        .logs
                        .send(log)
                        .map_err(|e| io::Error::new(ErrorKind::BrokenPipe, e))?;
                    Ok(())
                }
            })
    }

    #[instrument(level = "debug", skip_all)]
    fn flush(&self) -> io::Result<()> {
        self.flush_channel
            .try_lock()
            .map_err(|_| {
                io::Error::new(
                    ErrorKind::BrokenPipe,
                    LockError("Failed to acquire flush lock".to_string()),
                )
            })
            .and_then(|maybe_flush| match &*maybe_flush {
                None => Err(io::Error::new(ErrorKind::BrokenPipe, AdapterShutdownError)),
                Some(stream) => {
                    stream
                        .request
                        .send(())
                        .map_err(|e| io::Error::new(ErrorKind::BrokenPipe, e))?;
                    let r = stream
                        .response
                        .recv()
                        .map_err(|e| io::Error::new(ErrorKind::BrokenPipe, e))?;
                    r.map_err(|e| io::Error::new(ErrorKind::Other, e))
                }
            })
    }

    fn shutdown(&self) {
        if let Err(e) = self.flush() {
            log_error(e);
        }
        match self.flush_channel.try_lock() {
            Ok(mut flush) => std::mem::drop(flush.take()),
            Err(e) => log_error(e),
        }
        match self.log_channel.try_lock() {
            Ok(mut logs) => std::mem::drop(logs.take()),
            Err(e) => log_error(e),
        }
    }
}

impl Drop for DataDogAdapter {
    fn drop(&mut self) {
        self.shutdown()
    }
}
