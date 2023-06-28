use crate::test_subscriber::TestSubscriber;
use tempfile::TempDir;

pub struct AppHandle {
    _app_dir: TempDir,
    _seed_dir: TempDir,
    _handle: tokio::task::JoinHandle<()>,
    pub rx: TestSubscriber,
}

pub async fn run_app() -> AppHandle {
    let app_dir = TempDir::new().expect("Failed to create temporary directory");
    let seed_dir = TempDir::new().expect("Failed to create temporary directory");
    let _app_handle = {
        let as_string = |dir: &TempDir| {
            dir.path()
                .to_str()
                .expect("Could not convert path to string")
                .to_string()
        };

        let app_dir = as_string(&app_dir);
        let seed_dir = as_string(&seed_dir);
        tokio::task::spawn_blocking(move || {
            native::api::run(default_config(), app_dir, seed_dir).expect("Could not run app")
        })
    };

    let (rx, tx) = TestSubscriber::new();
    native::event::subscribe(tx);

    // TODO: Get rid of the sleep, but wait for the app to be initialised
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    AppHandle {
        _app_dir: app_dir,
        _seed_dir: seed_dir,
        _handle: _app_handle,
        rx,
    }
}

// Values taken from `environment.dart`
// TODO: move to default impl of Config
fn default_config() -> native::config::api::Config {
    native::config::api::Config {
        coordinator_pubkey: "02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9"
            .to_string(),
        esplora_endpoint: "http://127.0.0.1:3000".to_string(),
        host: "127.0.0.1".to_string(),
        p2p_port: 9045,
        http_port: 8000,
        network: "regtest".to_string(),
    }
}
