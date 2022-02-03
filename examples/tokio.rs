use flexi_logger_datadog::config::DataDogConfigBuilder;
use flexi_logger_datadog::init_tokio_logger;
use log::{debug, error, info, trace};

#[tokio::main]
async fn main() {
    let dd_config = DataDogConfigBuilder::new(
        "logger-example".to_string(),
        "logger-example".to_string(),
        "DUMMY_API_KEY".to_string(),
    )
    .build();
    let (logger_handle, _) = init_tokio_logger(dd_config, None).await.unwrap();
    trace!("Trace message");
    debug!("Debug message");
    info!("Info message");
    error!("Error message");
}
