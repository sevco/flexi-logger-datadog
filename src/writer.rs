//! Writer task that posts data to the api

use crate::error::Error::ChannelError;
use crate::error::{log_error, Error};
use crate::DataDogConfig;
use chrono::{DateTime, Duration, Utc};
use flume::RecvTimeoutError;
use itertools::Itertools;
use log::{debug, error};
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use std::time;

/// Default channel recv timeout
const POLL_TIMEOUT_MS: u64 = 100;

/// API writer
pub struct DataDogHttpWriter {
    /// HTTP client
    client: Client,
    /// DataDog api url
    api_host: String,
    /// DataDog api key
    api_key: String,
    /// Query path
    query: Vec<(String, String)>,
    /// Maximum allowed line sized
    max_line_size: usize,
    /// Maximum allowed request size
    max_payload_size: usize,
    /// How often to flush writer (never if [`None`])
    flush_interval: Option<Duration>,
    /// When logs were last flushed
    last_flushed: DateTime<Utc>,
    /// Log receiver
    logs: flume::Receiver<String>,
    /// Flush request receiver
    flush_request: flume::Receiver<()>,
    /// Flush response sender
    flush_response: flume::Sender<Result<(), Error>>,
    /// Log buffer
    buffer_lines: Vec<String>,
    /// Size of buffer
    buffer_size: usize,
}

impl DataDogHttpWriter {
    /// Create new [`DataDogHttpWriter`]
    pub fn new(
        datadog_config: DataDogConfig,
        flush_interval: Option<Duration>,
        logs: flume::Receiver<String>,
        flush_request: flume::Receiver<()>,
        flush_response: flume::Sender<Result<(), Error>>,
    ) -> Self {
        let query = vec![
            ("host".to_string(), datadog_config.hostname),
            ("service".to_string(), datadog_config.service),
            ("ddsource".to_string(), datadog_config.source),
            (
                "ddtags".to_string(),
                datadog_config
                    .tags
                    .into_iter()
                    .map(|(k, v)| format!("{}:{}", k, v))
                    .join(","),
            ),
        ];
        Self {
            client: Client::default(),
            api_host: datadog_config.api_host,
            api_key: datadog_config.api_key,
            query,
            max_line_size: datadog_config.max_line_size,
            max_payload_size: datadog_config.max_payload_size,
            flush_interval,
            last_flushed: Utc::now(),
            logs,
            flush_request,
            flush_response,
            buffer_lines: vec![],
            buffer_size: 0,
        }
    }

    /// Writer poll loop.
    ///
    /// This is what drives the actual execution of the logger
    pub async fn poll(&mut self) {
        let timeout = time::Duration::from_millis(POLL_TIMEOUT_MS);
        loop {
            // Check if a flush is necessary
            if let Err(e) = self.time_based_flush().await {
                log_error(e);
            }

            // Retrieve and handle any new log messages
            match self.receive_logs(timeout).await {
                Ok(true) => (),
                Ok(false) => break,
                Err(e) => log_error(e),
            }

            // Check for any flush requests
            match self.receive_flush(timeout).await {
                Ok(true) => (),
                Ok(false) => break,
                Err(e) => log_error(e),
            }
        }

        // Loop has been exited here from one or all of the channels closing
        // Drain and handle any remaining messages from the log channel and flush one last time
        if let Err(e) = self.drain().await {
            log_error(e);
        }
        if let Err(e) = self.flush().await {
            log_error(e);
        }
    }

    /// Receive and process any incoming log lines
    async fn receive_logs(&mut self, timeout: time::Duration) -> Result<bool, Error> {
        match self.logs.recv_timeout(timeout) {
            Ok(l) => {
                self.on_message(l).await;
                self.check_flush().await?;
                Ok(true)
            }
            Err(RecvTimeoutError::Timeout) => Ok(true),
            Err(RecvTimeoutError::Disconnected) => Ok(false),
        }
    }

    /// Receive and process any incoming flush requests
    async fn receive_flush(&mut self, timeout: time::Duration) -> Result<bool, Error> {
        match self.flush_request.recv_timeout(timeout / 2) {
            Ok(_) => {
                // On flush request, perform a flush and send the result back over the channel
                self.drain().await?;
                let flush_result = self.flush().await.map_err(|e| {
                    eprintln!("Failed to flush logs: {}", e);
                    e
                });
                self.flush_response
                    .send(flush_result)
                    .map_err(|e| ChannelError(format!("Failed to send flush response: {}", e)))?;
                Ok(true)
            }
            Err(RecvTimeoutError::Timeout) => Ok(true),
            Err(RecvTimeoutError::Disconnected) => Ok(false),
        }
    }

    /// Handle incoming log line
    async fn on_message(&mut self, message: String) {
        let bytes = message.as_bytes().len();
        if bytes > self.max_line_size {
            error!("Log line longer than 1MB maximum");
        } else {
            self.buffer_size += message.as_bytes().len();
            self.buffer_lines.push(message);
        }
    }

    /// Flush log lines in buffer
    async fn flush(&mut self) -> Result<(), Error> {
        if self.buffer_size > 0 {
            debug!("Flushing logger");
            self.send().await?;
            self.buffer_lines = vec![];
            self.buffer_size = 0;
            self.last_flushed = Utc::now();
        }
        Ok(())
    }

    /// Post data to api
    async fn send(&mut self) -> Result<(), Error> {
        debug!("Sending {} log lines", self.buffer_lines.len());
        match self
            .client
            .post(&self.api_host)
            .query(&self.query)
            .header("DD-API-KEY", &self.api_key)
            .header(CONTENT_TYPE, "text/plain")
            .body(self.buffer_lines.join("\n"))
            .send()
            .await
        {
            Ok(r) => {
                r.error_for_status()?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Check if flush interval has elapsed since last send, and flush if so
    async fn time_based_flush(&mut self) -> Result<(), Error> {
        if let Some(d) = self.flush_interval {
            if Utc::now() > self.last_flushed + d {
                self.flush().await?;
            }
        }
        Ok(())
    }

    /// Drain and handle any messages on the log channel
    async fn drain(&mut self) -> Result<(), Error> {
        let drained = self.logs.drain().collect_vec();
        for message in drained {
            self.on_message(message).await;
            self.check_flush().await?;
        }
        Ok(())
    }

    /// Check if buffer
    async fn check_flush(&mut self) -> Result<(), Error> {
        if self.buffer_size >= self.max_payload_size {
            self.flush().await
        } else {
            Ok(())
        }
    }
}
