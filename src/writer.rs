use crate::error::{log_error, Error};
use crate::DataDogConfig;
use chrono::{DateTime, Duration, Utc};
use flume::RecvTimeoutError;
use itertools::Itertools;
use log::{error, info};
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use std::time;

const POLL_TIMEOUT_MS: u64 = 100;

pub struct DataDogHttpWriter {
    client: Client,
    api_host: String,
    api_key: String,
    query: Vec<(String, String)>,
    max_line_size: usize,
    max_payload_size: usize,
    flush_interval: Option<Duration>,
    last_flushed: DateTime<Utc>,
    logs: flume::Receiver<String>,
    flush_request: flume::Receiver<()>,
    flush_response: flume::Sender<Result<(), Error>>,
    buffer_lines: Vec<String>,
    buffer_size: usize,
}

impl DataDogHttpWriter {
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

    async fn buffer(&mut self, line: String) -> Result<(), Error> {
        self.buffer_size += line.as_bytes().len();
        self.buffer_lines.push(line);
        if self.buffer_size >= self.max_payload_size {
            self.flush().await
        } else {
            Ok(())
        }
    }

    async fn on_message(&mut self, message: String) -> Result<(), Error> {
        let bytes = message.as_bytes().len();
        if bytes > self.max_line_size {
            error!("Log line longer than 1MB maximum");
            eprintln!("{}", message);
            Ok(())
        } else {
            self.buffer(message).await.map_err(Error::from)
        }
    }

    async fn flush(&mut self) -> Result<(), Error> {
        info!("Flushing logger");
        if self.buffer_size > 0 {
            self.send().await?;
            self.buffer_lines = vec![];
            self.buffer_size = 0;
            self.last_flushed = Utc::now();
        }
        Ok(())
    }

    async fn send(&mut self) -> Result<(), Error> {
        info!("Sending {} log lines", self.buffer_lines.len());
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

    async fn time_based_flush(&mut self) -> Result<(), Error> {
        if let Some(d) = self.flush_interval {
            if Utc::now() > self.last_flushed + d {
                self.flush().await?;
            }
        }
        Ok(())
    }

    pub async fn poll(&mut self) {
        let timeout = time::Duration::from_millis(POLL_TIMEOUT_MS);
        loop {
            if let Err(e) = self.time_based_flush().await {
                log_error(e);
            }

            match self.logs.recv_timeout(timeout) {
                Ok(l) => match self.on_message(l).await {
                    Ok(_) => (),
                    Err(e) => {
                        eprintln!("Failed to write logs: {}", e);
                    }
                },
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => break,
            };

            match self.flush_request.recv_timeout(timeout) {
                Ok(_) => {
                    let flush_result = self.flush().await.map_err(|e| {
                        eprintln!("Failed to flush logs: {}", e);
                        e
                    });
                    if let Err(e) = self.flush_response.send(flush_result) {
                        log_error(e);
                    }
                }
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => break,
            };
        }
        for message in self.logs.drain().collect_vec() {
            if let Err(e) = self.on_message(message).await {
                log_error(e);
            }
        }
        if let Err(e) = self.flush().await {
            log_error(e);
        }
    }
}
