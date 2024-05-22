use crate::commons::reqwest_client;
use crate::config;
use crate::dlc::get_node_key;
use crate::dlc::get_node_pubkey;
use anyhow::Result;
use bitcoin::Amount;
use reqwest::Url;
use xxi_node::commons;

pub async fn get_hodl_invoice_from_coordinator(amount: Amount) -> Result<String> {
    // TODO: store the preimage in the node so that we can remember it
    let pre_image = commons::create_pre_image();

    let client = reqwest_client();
    let url = format!("http://{}", config::get_http_endpoint());
    let url = Url::parse(&url).expect("correct URL");
    let url = url.join("/api/invoice")?;

    let invoice_params = commons::HodlInvoiceParams {
        trader_pubkey: get_node_pubkey(),
        amt_sats: amount.to_sat(),
        r_hash: pre_image.hash,
    };
    let invoice_params = commons::SignedValue::new(invoice_params, get_node_key())?;

    let response = client
        .post(url)
        .json(&invoice_params)
        .send()
        .await?
        .error_for_status()?;

    let payment_request = response.json::<String>().await?;
    Ok(payment_request)
}
