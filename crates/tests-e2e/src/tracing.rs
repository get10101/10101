use std::sync::Once;

pub fn init_tracing() {
    static TRACING_TEST_SUBSCRIBER: Once = Once::new();

    TRACING_TEST_SUBSCRIBER.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(
                "debug,\
                 hyper=warn,\
                 reqwest=warn,\
                 rustls=warn,\
                 bdk=info,\
                 lightning::ln::peer_handler=debug,\
                 lightning=trace,\
                 sled=info,\
                 ureq=info",
            )
            .with_test_writer()
            .init()
    })
}
