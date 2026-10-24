use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time;

use super::manager::{Language, StopReason};

const DEFAULT_CHECK_INTERVAL_SECS: u64 = 60;

pub struct IdleMonitor {
    last_used: Mutex<HashMap<Language, Instant>>,
    check_interval: Duration,
}

impl IdleMonitor {
    pub fn new() -> Self {
        Self {
            last_used: Mutex::new(HashMap::new()),
            check_interval: Duration::from_secs(DEFAULT_CHECK_INTERVAL_SECS),
        }
    }

    pub async fn touch(&self, language: Language) {
        let mut last_used = self.last_used.lock().await;
        last_used.insert(language, Instant::now());
    }

    pub async fn run(self: Arc<Self>, manager: Arc<super::manager::LanguageServerManager>) {
        let mut interval = time::interval(self.check_interval);
        loop {
            interval.tick().await;
            self.check_and_shutdown_idle(&manager).await;
        }
    }

    async fn check_and_shutdown_idle(&self, manager: &super::manager::LanguageServerManager) {
        let mut last_used = self.last_used.lock().await;
        let languages: Vec<Language> = last_used.keys().copied().collect();

        for language in languages {
            let idle_timeout = manager
                .config_for(language)
                .map(|cfg| Duration::from_secs(cfg.idle_timeout_secs))
                .unwrap_or(Duration::from_secs(300));

            let should_shutdown = match last_used.get(&language) {
                Some(last) => last.elapsed() >= idle_timeout,
                None => false,
            };

            if should_shutdown {
                tracing::info!(language = ?language, "shutting down idle server");
                last_used.remove(&language);
                drop(last_used);

                if let Err(e) = manager
                    .stop_server_with_reason(language, StopReason::IdleTimeout)
                    .await
                {
                    tracing::warn!(language = ?language, error = %e, "error shutting down idle server");
                }

                last_used = self.last_used.lock().await;
            }
        }
    }
}
