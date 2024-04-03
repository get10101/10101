use crate::config::ConfigInternal;
use crate::ln_dlc::node::Node;
use crate::logger::LogEntry;
use crate::storage::TenTenOneNodeStorage;
use anyhow::Result;
use commons::OrderbookRequest;
use commons::TenTenOneConfig;
use flutter_rust_bridge::StreamSink;
use ln_dlc_node::seed::Bip39Seed;
use parking_lot::RwLock;
use state::Storage;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::broadcast::Sender;

/// For testing we need the state to be mutable as otherwise we can't start another app after
/// stopping the first one. Note, running two apps at the same time will not work as the states
/// below are static and will be used for both apps.
/// TODO(holzeis): Check if there is a way to bind the state to the lifetime of the app (node).

static CONFIG: Storage<RwLock<ConfigInternal>> = Storage::new();
static NODE: Storage<RwLock<Arc<Node>>> = Storage::new();
static SEED: Storage<RwLock<Bip39Seed>> = Storage::new();
static STORAGE: Storage<RwLock<TenTenOneNodeStorage>> = Storage::new();
static RUNTIME: Storage<Runtime> = Storage::new();
static WEBSOCKET: Storage<RwLock<Sender<OrderbookRequest>>> = Storage::new();
static LOG_STREAM_SINK: Storage<RwLock<Arc<StreamSink<LogEntry>>>> = Storage::new();
static TENTENONE_CONFIG: Storage<RwLock<TenTenOneConfig>> = Storage::new();

pub fn set_config(config: ConfigInternal) {
    match CONFIG.try_get() {
        Some(c) => *c.write() = config,
        None => {
            CONFIG.set(RwLock::new(config));
        }
    }
}

pub fn get_config() -> ConfigInternal {
    CONFIG.get().read().clone()
}

pub fn set_node(node: Arc<Node>) {
    match NODE.try_get() {
        Some(n) => *n.write() = node,
        None => {
            NODE.set(RwLock::new(node));
        }
    }
}

pub fn get_node() -> Arc<Node> {
    NODE.get().read().clone()
}

pub fn try_get_node() -> Option<Arc<Node>> {
    NODE.try_get().map(|n| n.read().clone())
}

pub fn set_seed(seed: Bip39Seed) {
    match SEED.try_get() {
        Some(s) => *s.write() = seed,
        None => {
            SEED.set(RwLock::new(seed));
        }
    }
}

pub fn get_seed() -> Bip39Seed {
    SEED.get().read().clone()
}

pub fn try_get_seed() -> Option<Bip39Seed> {
    SEED.try_get().map(|s| s.read().clone())
}

pub fn set_storage(storage: TenTenOneNodeStorage) {
    match STORAGE.try_get() {
        Some(s) => *s.write() = storage,
        None => {
            STORAGE.set(RwLock::new(storage));
        }
    }
}

pub fn get_storage() -> TenTenOneNodeStorage {
    STORAGE.get().read().clone()
}

pub fn try_get_storage() -> Option<TenTenOneNodeStorage> {
    STORAGE.try_get().map(|s| s.read().clone())
}

/// Lazily creates a multi threaded runtime with the the number of worker threads corresponding to
/// the number of available cores.
pub fn get_or_create_tokio_runtime() -> Result<&'static Runtime> {
    if RUNTIME.try_get().is_none() {
        let runtime = Runtime::new()?;
        RUNTIME.set(runtime);
    }

    Ok(RUNTIME.get())
}

pub fn set_websocket(websocket: Sender<OrderbookRequest>) {
    match WEBSOCKET.try_get() {
        Some(s) => *s.write() = websocket,
        None => {
            WEBSOCKET.set(RwLock::new(websocket));
        }
    }
}

pub fn get_websocket() -> Sender<OrderbookRequest> {
    WEBSOCKET.get().read().clone()
}

pub fn try_get_websocket() -> Option<Sender<OrderbookRequest>> {
    WEBSOCKET.try_get().map(|w| w.read().clone())
}

pub fn set_log_stream_sink(sink: Arc<StreamSink<LogEntry>>) {
    match LOG_STREAM_SINK.try_get() {
        Some(l) => *l.write() = sink,
        None => {
            LOG_STREAM_SINK.set(RwLock::new(sink));
        }
    }
}

pub fn try_get_log_stream_sink() -> Option<Arc<StreamSink<LogEntry>>> {
    LOG_STREAM_SINK.try_get().map(|l| l.read().clone())
}

pub fn set_tentenone_config(config: TenTenOneConfig) {
    match TENTENONE_CONFIG.try_get() {
        None => {
            TENTENONE_CONFIG.set(RwLock::new(config));
        }
        Some(s) => {
            *s.write() = config;
        }
    }
}

pub fn try_get_tentenone_config() -> Option<TenTenOneConfig> {
    TENTENONE_CONFIG.try_get().map(|w| w.read().clone())
}
