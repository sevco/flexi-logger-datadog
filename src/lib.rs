//! DataDog output for [flexi_logger](https://github.com/emabee/flexi_logger)

#![deny(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

use crate::adapter::DataDogAdapter;
use crate::config::DataDogConfig;
use crate::writer::DataDogHttpWriter;
use chrono::Duration;
use flexi_logger::{FlexiLoggerError, Logger, LoggerHandle};
#[cfg(feature = "tokio-rt")]
use tokio::task::JoinHandle;

pub mod adapter;
pub mod config;
pub mod error;
pub mod writer;

/// Create and set logger with the writer running on the tokio runtime
#[cfg(feature = "tokio-rt")]
pub async fn init_tokio_logger(
    datadog_config: DataDogConfig,
    flush_interval: Option<Duration>,
) -> Result<(LoggerHandle, JoinHandle<()>), FlexiLoggerError> {
    let (adapter, handle) = spawn_tokio_logger(datadog_config, flush_interval).await;
    Logger::try_with_env()?
        .log_to_writer(Box::new(adapter))
        .start()
        .map(|l| (l, handle))
}

/// Create and spawn logger on the tokio runtime
#[cfg(feature = "tokio-rt")]
pub async fn spawn_tokio_logger(
    datadog_config: DataDogConfig,
    flush_interval: Option<Duration>,
) -> (DataDogAdapter, JoinHandle<()>) {
    let (adapter, mut writer) = new_datadog_http_logger(datadog_config, flush_interval);
    let handle = tokio::spawn(async move { writer.poll().await });
    (adapter, handle)
}

/// Create [`DataDogAdapter`] and [`DataDogHttpWriter`].
/// `writer.poll()` will need to be spawned via a thread or runtime
pub fn new_datadog_http_logger(
    datadog_config: DataDogConfig,
    flush_interval: Option<Duration>,
) -> (DataDogAdapter, DataDogHttpWriter) {
    let (log_sender, log_receiver) = flume::unbounded();
    let (flush_request_sender, flush_request_receiver) = flume::bounded(0);
    let (flush_response_sender, flush_response_receiver) = flume::bounded(0);
    let adapter = DataDogAdapter::new(log_sender, flush_request_sender, flush_response_receiver);
    let writer = DataDogHttpWriter::new(
        datadog_config,
        flush_interval,
        log_receiver,
        flush_request_receiver,
        flush_response_sender,
    );
    (adapter, writer)
}

#[cfg(test)]
mod tests {
    use crate::config::{DataDogConfig, DataDogConfigBuilder};
    use crate::error::Error;
    use crate::{spawn_tokio_logger, DataDogAdapter};
    use anyhow::Result;
    use chrono::Duration;
    use flexi_logger::writers::LogWriter;
    use flexi_logger::DeferredNow;
    use httpmock::{Mock, MockServer};
    use itertools::Itertools;
    use log::{Level, Record};
    use std::fmt::Arguments;
    use std::future::Future;
    use std::thread::sleep;
    use std::time;
    use tokio::task::JoinHandle;

    fn record(level: Level, args: Arguments) -> Record {
        Record::builder().level(level).args(args).build()
    }

    fn dd_config(host: String) -> DataDogConfigBuilder {
        let mut builder = DataDogConfigBuilder::new(
            "host".to_string(),
            "test".to_string(),
            "dummy_key".to_string(),
        );
        builder
            .with_tags(vec![("test_key", "test_value")])
            .with_gzip(Some(false))
            .with_api_host(Some(host));
        builder
    }

    fn mock<'a>(server: &'a MockServer, lines: Vec<&str>) -> Mock<'a> {
        server.mock(|when, then| {
            when.query_param("host", "host")
                .query_param("service", "test")
                .query_param("ddsource", "rust")
                .query_param("ddtags", "test_key:test_value")
                .body(lines.join("\n"));
            then.status(200);
        })
    }

    async fn with_logger<F, Fut>(
        config: DataDogConfig,
        flush_interval: Option<Duration>,
        f: F,
    ) -> Result<JoinHandle<()>, Error>
    where
        F: FnOnce(DataDogAdapter) -> Fut,
        Fut: Future<Output = Result<(), Error>>,
    {
        let (adapter, handle) = spawn_tokio_logger(config, flush_interval).await;
        f(adapter).await.map(|_| handle)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_drop() -> Result<()> {
        let server = MockServer::start();
        let mock = mock(&server, vec!["DEBUG [] this is a test\n"]);

        with_logger(
            dd_config(server.base_url()).build(),
            None,
            |logger| async move {
                logger.write(
                    &mut DeferredNow::new(),
                    &record(Level::Debug, format_args!("this is a test")),
                )?;
                Ok(())
            },
        )
        .await?
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_flush() -> Result<()> {
        let server = MockServer::start();
        let mock = mock(&server, vec!["DEBUG [] this is a test\n"]);

        with_logger(
            dd_config(server.base_url()).build(),
            None,
            |logger| async move {
                logger.write(
                    &mut DeferredNow::new(),
                    &record(Level::Debug, format_args!("this is a test")),
                )?;
                logger.flush()?;
                mock.assert();
                Ok(())
            },
        )
        .await?
        .await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_max_payload() -> Result<()> {
        let server = MockServer::start();
        let mocks = (0..3)
            .map(|i| mock(&server, vec![&format!("DEBUG [] this is a test {}\n", i)]))
            .collect_vec();

        let mut dd_config = dd_config(server.base_url());
        dd_config.with_max_payload_size(Some("DEBUG [] this is a test 0\n".len()));

        with_logger(dd_config.build(), None, |logger| async move {
            for i in 0..3 {
                logger.write(
                    &mut DeferredNow::new(),
                    &record(Level::Debug, format_args!("this is a test {}", i)),
                )?;
            }
            Ok(())
        })
        .await?
        .await?;
        for mock in mocks {
            mock.assert();
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_schedule() -> Result<()> {
        let server = MockServer::start();
        let mock = mock(&server, vec!["DEBUG [] this is a test\n"]);

        with_logger(
            dd_config(server.base_url()).build(),
            Some(Duration::milliseconds(100)),
            |logger| async move {
                logger.write(
                    &mut DeferredNow::new(),
                    &record(Level::Debug, format_args!("this is a test")),
                )?;
                sleep(time::Duration::from_millis(500));
                mock.assert();
                Ok(())
            },
        )
        .await?
        .await?;
        Ok(())
    }
}
