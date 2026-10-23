use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

static LOGGER_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub struct LogGuard {
    _guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

impl LogGuard {
    pub fn none() -> Self {
        Self { _guard: None }
    }
}

pub fn init_structured_logging(log_dir: &Path) -> Result<LogGuard> {
    if LOGGER_INITIALIZED.swap(true, Ordering::SeqCst) {
        return Ok(LogGuard::none());
    }

    std::fs::create_dir_all(log_dir)?;

    let file_appender = tracing_appender::rolling::daily(log_dir, "leadfinder.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,leadfinder=debug"));

    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(non_blocking);

    let console_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_target(false)
        .with_writer(std::io::stderr);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(console_layer)
        .try_init()
        .ok();

    Ok(LogGuard {
        _guard: Some(guard),
    })
}
