use reqwest::Client;

pub fn init_reqwest() -> Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Could not build reqwest client")
}
