use bitmex_client::client::Client;
use bitmex_client::models::ContractSymbol;
use bitmex_client::models::Network;
use bitmex_client::models::Side;

#[tokio::main]
async fn main() {
    let api_key = "some_api_key";
    let api_secret = "some_secret";

    let client = Client::new(Network::Testnet).with_credentials(api_key, api_secret);
    let _order = client
        .create_order(
            ContractSymbol::XbtUsd,
            100,
            Side::Buy,
            Some("example".to_string()),
        )
        .await
        .expect("To be able to post order");

    let _positions = client
        .positions()
        .await
        .expect("To be able to get positions");
}
