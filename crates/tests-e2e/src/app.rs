use crate::test_subscriber::TestSubscriber;
use crate::test_subscriber::ThreadSafeSenders;
use crate::wait_until;
use native::api;
use tempfile::TempDir;

pub struct AppHandle {
    pub rx: TestSubscriber,
    _app_dir: TempDir,
    _seed_dir: TempDir,
    _handle: tokio::task::JoinHandle<()>,
    _tx: ThreadSafeSenders,
}

impl AppHandle {
    pub fn stop(&self) {
        self._handle.abort()
    }
}

pub async fn run_app(seed_phrase: Option<Vec<String>>) -> AppHandle {
    let app_dir = TempDir::new().unwrap();
    let seed_dir = TempDir::new().unwrap();
    let _app_handle = {
        let as_string = |dir: &TempDir| dir.path().to_str().unwrap().to_string();

        let app_dir = as_string(&app_dir);
        let seed_dir = as_string(&seed_dir);

        native::api::set_config(test_config(), app_dir, seed_dir.clone()).unwrap();

        if let Some(seed_phrase) = seed_phrase {
            tokio::task::spawn_blocking({
                let seed_dir = seed_dir.clone();
                move || {
                    api::restore_from_seed_phrase(
                        seed_phrase.join(" "),
                        format!("{seed_dir}/regtest/seed"),
                    )
                    .unwrap();
                }
            })
            .await
            .unwrap();
        }

        tokio::task::spawn_blocking(move || {
            native::api::run(
                seed_dir,
                "".to_string(),
                native::api::IncludeBacktraceOnPanic::No,
            )
            .unwrap()
        })
    };

    let (rx, tx) = TestSubscriber::new().await;
    let app = AppHandle {
        _app_dir: app_dir,
        _seed_dir: seed_dir,
        _handle: _app_handle,
        rx,
        _tx: tx.clone(),
    };

    native::event::subscribe(tx);

    wait_until!(app.rx.init_msg() == Some("10101 is ready.".to_string()));
    wait_until!(app.rx.wallet_info().is_some()); // wait for initial wallet sync
    app
}

// Values mostly taken from `environment.dart`
fn test_config() -> native::config::api::Config {
    native::config::api::Config {
        coordinator_pubkey: "02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9"
            .to_string(),
        esplora_endpoint: "http://127.0.0.1:3000".to_string(),
        host: "127.0.0.1".to_string(),
        p2p_port: 9045,
        http_port: 8000,
        network: "regtest".to_string(),
        oracle_endpoint: "http://127.0.0.1:8081".to_string(),
        oracle_pubkey: "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0"
            .to_string(),
        health_check_interval_secs: 1, // We want to measure health more often in tests
    }
}
