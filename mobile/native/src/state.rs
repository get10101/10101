use crate::config::ConfigInternal;
use crate::ln_dlc::node::Node;
use crate::storage::TenTenOneNodeStorage;
use anyhow::Result;
use ln_dlc_node::seed::Bip39Seed;
use parking_lot::RwLock;
use state::Storage;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// For testing we need the state to be mutable as otherwise we can't start another app after
/// stopping the first one. Note, running two apps at the same time will not work as the states
/// below are static and will be used for both apps.
/// TODO(holzeis): Check if there is a way to bind the state to the lifetime of the app (node).

static CONFIG: Storage<RwLock<ConfigInternal>> = Storage::new();
static NODE: Storage<RwLock<Arc<Node>>> = Storage::new();
static SEED: Storage<RwLock<Bip39Seed>> = Storage::new();
static STORAGE: Storage<RwLock<TenTenOneNodeStorage>> = Storage::new();

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
    static RUNTIME: Storage<Runtime> = Storage::new();

    if RUNTIME.try_get().is_none() {
        let runtime = Runtime::new()?;
        RUNTIME.set(runtime);
    }

    Ok(RUNTIME.get())
}
