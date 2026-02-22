use rustcnc_core::config::LoggingConfig;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

pub struct LoggingGuards {
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

/// Initialize logging based on config + optional CLI override.
///
/// Order of precedence for level:
/// 1) `RUST_LOG` env var (if present / valid)
/// 2) CLI `--log-level` (if provided)
/// 3) `config.logging.level`
pub fn init_logging(
    config: &LoggingConfig,
    level_override: Option<&str>,
) -> anyhow::Result<LoggingGuards> {
    let level = level_override.unwrap_or(&config.level);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    let mut guards = LoggingGuards { _file_guard: None };

    let mut file_enabled = false;
    let file_layer =
        config
            .log_dir
            .as_ref()
            .and_then(|log_dir| match std::fs::create_dir_all(log_dir) {
                Ok(()) => {
                    let file_appender = tracing_appender::rolling::daily(log_dir, "rustcnc.log");
                    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
                    guards._file_guard = Some(guard);
                    file_enabled = true;

                    Some(
                        fmt::layer()
                            .with_writer(non_blocking)
                            .with_ansi(false)
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_file(false),
                    )
                }
                Err(e) => {
                    eprintln!(
                        "Failed to create log_dir {}: {} (falling back to console logging)",
                        log_dir, e
                    );
                    None
                }
            });

    let console_layer = (config.console_output || !file_enabled).then(|| {
        fmt::layer()
            .with_writer(std::io::stdout)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(false)
    });

    Registry::default()
        .with(filter)
        .with(file_layer)
        .with(console_layer)
        .init();

    Ok(guards)
}
