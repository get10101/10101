use flutter_rust_bridge::SyncReturn;

pub fn hello_world() -> SyncReturn<String> {
    SyncReturn("Hello World".to_string())
}