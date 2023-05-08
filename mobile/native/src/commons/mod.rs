pub mod api;

/// Provide a reqwest client with a specified 10 seconds timeout.
//
// FIXME: Ideally, we should reuse the same reqwest client for all requests.
pub fn reqwest_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to build reqwest client")
}
