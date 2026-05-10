use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time;

use super::manager::{Language, LanguageServerManager};

const DEFAULT_CHECK_INTERVAL_SECS: u64 = 60;

pub struct IdleMonitor {
    last_used: Mutex<HashMap<Language, Instant>>,
    manager: Arc<LanguageServerManager>,
    check_interval: Duration,
}

impl IdleMonitor {
    pub fn new(manager: Arc<LanguageServerManager>) -> Self {
        Self {
            last_used: Mutex::new(HashMap::new()),
            manager,
            check_interval: Duration::from_secs(DEFAULT_CHECK_INTERVAL_SECS),
        }
    }

    pub fn with_check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }

    /// Update the last-used timestamp for a language (called on each tool invocation).
    pub async fn touch(&self, language: Language) {
        let mut last_used = self.last_used.lock().await;
        last_used.insert(language, Instant::now());
    }

    /// Run the idle monitor as a background task.
    /// Periodically checks for idle servers and shuts them down.
    pub async fn run(self: Arc<Self>) {
        let mut interval = time::interval(self.check_interval);
        loop {
            interval.tick().await;
            self.check_and_shutdown_idle().await;
        }
    }

    async fn check_and_shutdown_idle(&self) {
        let mut last_used = self.last_used.lock().await;
        let languages: Vec<Language> = last_used.keys().copied().collect();

        for language in languages {
            let idle_timeout = self
                .manager
                .config_for(language)
                .map(|cfg| Duration::from_secs(cfg.idle_timeout_secs))
                .unwrap_or(Duration::from_secs(300));

            let should_shutdown = match last_used.get(&language) {
                Some(last) => last.elapsed() >= idle_timeout,
                None => false,
            };

            if should_shutdown {
                eprintln!("[idle-monitor] shutting down idle {language:?} server");
                last_used.remove(&language);
                drop(last_used);

                if let Err(e) = self.manager.stop_server(language).await {
                    eprintln!("[idle-monitor] error shutting down {language:?}: {e}");
                }

                last_used = self.last_used.lock().await;
            }
        }
    }
}
