use reqwest::Client;

pub fn init_reqwest() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap()
}
