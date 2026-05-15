use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time;

use super::manager::Language;
use crate::stats::StatsRecorder;

const DEFAULT_CHECK_INTERVAL_SECS: u64 = 60;

pub struct IdleMonitor {
    last_used: Mutex<HashMap<Language, Instant>>,
    check_interval: Duration,
    stats: Arc<StatsRecorder>,
}

impl IdleMonitor {
    pub fn new(stats: Arc<StatsRecorder>) -> Self {
        Self {
            last_used: Mutex::new(HashMap::new()),
            check_interval: Duration::from_secs(DEFAULT_CHECK_INTERVAL_SECS),
            stats,
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
                eprintln!("[idle-monitor] shutting down idle {language:?} server");
                last_used.remove(&language);
                drop(last_used);

                if let Err(e) = manager.stop_server(language).await {
                    eprintln!("[idle-monitor] error shutting down {language:?}: {e}");
                }

                let lang_str = format!("{language:?}");
                if let Err(e) = self
                    .stats
                    .record_server_event(&lang_str, "stopped", None, Some("idle_timeout"))
                    .await
                {
                    eprintln!("stats recording failed: {e}");
                }

                last_used = self.last_used.lock().await;
            }
        }
    }
}
