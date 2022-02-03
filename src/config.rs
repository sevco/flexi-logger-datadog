//! Configuration structs
//! Defaults pulled from https://docs.datadoghq.com/api/latest/logs/#send-logs

use itertools::Itertools;

/// Default log api URL
const DEFAULT_DATADOG_INGEST_URL: &str = "https://http-intake.logs.datadoghq.com/api/v2/logs";
/// Maximum request size DataDog api will accept
const DEFAULT_MAX_PAYLOAD_BYTES: usize = 5000;
/// Maximum bytes to buffer before sending to DataDog
const DEFAULT_BODY_SEND_BYTES: usize = ((DEFAULT_MAX_PAYLOAD_BYTES as f64) * 0.75f64) as usize;
/// Maximum size of log line DataDog api will accept
const DEFAULT_MAX_LINE_BYTES: usize = 1024;

/// DataDog api configuration
pub struct DataDogConfig {
    /// The name of the originating host of the log
    pub hostname: String,
    /// The name of the application or service generating log events
    pub service: String,
    /// DataDog api key
    pub api_key: String,
    /// DataDog api url
    pub api_host: String,
    /// Tags associated with logs
    pub tags: Vec<(String, String)>,
    /// The integration name associated with your log
    pub source: String,
    /// Maximum allowed size of log line
    pub max_line_size: usize,
    /// Maximum allowed api request size
    pub max_payload_size: usize,
}

/// Builder for [`DataDogConfig`]
pub struct DataDogConfigBuilder {
    /// The name of the originating host of the log
    hostname: String,
    /// The name of the application or service generating log events
    service: String,
    /// DataDog api key
    api_key: String,
    /// DataDog api url
    api_host: Option<String>,
    /// Tags associated with logs
    tags: Vec<(String, String)>,
    /// The integration name associated with your log
    source: String,
    /// Maximum allowed size of log line
    max_line_size: Option<usize>,
    /// Maximum allowed api request size
    max_payload_size: Option<usize>,
}

impl DataDogConfigBuilder {
    /// Create new [`DataDogConfigBuilder`]
    pub fn new(hostname: String, service: String, api_key: String) -> Self {
        Self {
            hostname,
            service,
            api_key,
            api_host: None,
            tags: vec![],
            source: "rust".to_string(),
            max_line_size: None,
            max_payload_size: None,
        }
    }

    /// Configure api uri
    pub fn with_api_host(&mut self, api_host: Option<String>) -> &mut Self {
        self.api_host = api_host;
        self
    }

    /// Configure tags that will be applied to logs
    pub fn with_tags<S, T>(&mut self, tags: Vec<(S, T)>) -> &mut Self
    where
        String: From<S>,
        String: From<T>,
    {
        self.tags = tags
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect_vec();
        self
    }

    /// Configure source
    pub fn with_source(&mut self, source: String) -> &mut Self {
        self.source = source;
        self
    }

    /// Configure max line size
    pub fn with_max_line_size(&mut self, bytes: Option<usize>) -> &mut Self {
        self.max_line_size = bytes;
        self
    }

    /// Configure max payload size
    pub fn with_max_payload_size(&mut self, bytes: Option<usize>) -> &mut Self {
        self.max_payload_size = bytes;
        self
    }

    /// Build [`DataDogConfig`]
    pub fn build(&self) -> DataDogConfig {
        DataDogConfig {
            hostname: self.hostname.to_owned(),
            service: self.service.to_owned(),
            api_key: self.api_key.to_owned(),
            api_host: self
                .api_host
                .as_ref()
                .map(|s| s.to_owned())
                .unwrap_or_else(|| DEFAULT_DATADOG_INGEST_URL.to_string()),
            tags: self.tags.to_owned(),
            source: self.source.to_owned(),
            max_line_size: self
                .max_line_size
                .as_ref()
                .map(|s| s.to_owned())
                .unwrap_or(DEFAULT_MAX_LINE_BYTES),
            max_payload_size: self
                .max_payload_size
                .as_ref()
                .map(|s| s.to_owned())
                .unwrap_or(DEFAULT_BODY_SEND_BYTES),
        }
    }
}
