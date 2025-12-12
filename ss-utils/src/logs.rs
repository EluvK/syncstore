use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LogConfig {
    enable_debug: bool,
    directory: Option<String>,
    prefix: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            enable_debug: false,
            directory: Some("./".to_string()),
            prefix: "ss-utils".to_string(),
        }
    }
}

pub fn enable_log(config: &LogConfig) -> anyhow::Result<impl Drop> {
    let file_path = Path::new(config.directory.as_deref().unwrap_or("./")).join("logs");
    let log_prefix = config.prefix.clone();
    let log_level = if config.enable_debug { "debug" } else { "info" };

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(&log_prefix)
        .filename_suffix("log")
        .build(file_path)?;

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let time_offset = time::macros::offset!(+8);
    let time_format =
        time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]+08:00")
            .expect("time format should be valid");
    let timer = tracing_subscriber::fmt::time::OffsetTime::new(time_offset, time_format);

    let mut subscriber = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_timer(timer)
        .with_ansi(false);
    if config.enable_debug {
        subscriber = subscriber.with_max_level(tracing::Level::DEBUG);
    }
    tracing::subscriber::set_global_default(subscriber.finish())
        .map_err(|e| anyhow::anyhow!("Failed to set global default subscriber: {}", e))?;
    tracing::info!("Logging enabled with level: {}", log_level);

    Ok(_guard)
}
