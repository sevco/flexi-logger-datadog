use itertools::Itertools;

const DATADOG_INGEST_URL: &str = "https://http-intake.logs.datadoghq.com/api/v2/logs";
const MAX_PAYLOAD_BYTES: usize = 5000;
const BODY_SEND_BYTES: usize = ((MAX_PAYLOAD_BYTES as f64) * 0.75f64) as usize;
const MAX_LINE_BYTES: usize = 1024;

pub struct DataDogConfig {
    pub hostname: String,
    pub service: String,
    pub api_key: String,
    pub api_host: String,
    pub tags: Vec<(String, String)>,
    pub source: String,
    pub max_line_size: usize,
    pub max_payload_size: usize,
}

pub struct DataDogConfigBuilder {
    hostname: String,
    service: String,
    api_key: String,
    api_host: Option<String>,
    tags: Vec<(String, String)>,
    source: String,
    max_line_size: Option<usize>,
    max_payload_size: Option<usize>,
}

impl DataDogConfigBuilder {
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

    pub fn with_api_host(&mut self, api_host: Option<String>) -> &mut Self {
        self.api_host = api_host;
        self
    }

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

    pub fn with_source(&mut self, source: String) -> &mut Self {
        self.source = source;
        self
    }

    pub fn with_max_line_size(&mut self, bytes: Option<usize>) -> &mut Self {
        self.max_line_size = bytes;
        self
    }

    pub fn with_max_payload_size(&mut self, bytes: Option<usize>) -> &mut Self {
        self.max_payload_size = bytes;
        self
    }

    pub fn build(&self) -> DataDogConfig {
        DataDogConfig {
            hostname: self.hostname.to_owned(),
            service: self.service.to_owned(),
            api_key: self.api_key.to_owned(),
            api_host: self
                .api_host
                .as_ref()
                .map(|s| s.to_owned())
                .unwrap_or_else(|| DATADOG_INGEST_URL.to_string()),
            tags: self.tags.to_owned(),
            source: self.source.to_owned(),
            max_line_size: self
                .max_line_size
                .as_ref()
                .map(|s| s.to_owned())
                .unwrap_or(MAX_LINE_BYTES),
            max_payload_size: self
                .max_payload_size
                .as_ref()
                .map(|s| s.to_owned())
                .unwrap_or(BODY_SEND_BYTES),
        }
    }
}
