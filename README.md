# flexi-logger-datadog

![Crates.io](https://img.shields.io/crates/v/flexi-logger-datadog)
![docs.rs](https://docs.rs/flexi_logger_datadog/badge.svg)
![GitHub Workflow Status](https://img.shields.io/github/workflow/status/sevco/flexi-logger-datadog/Rust)

### Logger for https://github.com/emabee/flexi_logger that writes to DataDog.

## Usage

### Using tokio

```rust
#[tokio::main]
async fn main() {
    let dd_config = DataDogConfigBuilder::new(
        "logging-service-hostname".to_string(),
        "logging-service".to_string(),
        "DUMMY_API_KEY".to_string(),
    )
    .build();
    
    init_tokio_logger(dd_config, None).await.unwrap();
    
    trace!("Trace message");
    debug!("Debug message");
    info!("Info message");
    error!("Error message");
}
```