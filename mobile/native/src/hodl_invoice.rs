use crate::commons::reqwest_client;
use crate::config;
use crate::dlc::get_node_key;
use crate::dlc::get_node_pubkey;
use anyhow::Result;
use bitcoin::Amount;
use reqwest::Url;
use xxi_node::commons;

pub struct HodlInvoice {
    pub payment_request: String,
    pub pre_image: String,
    pub r_hash: String,
    pub amt_sats: Amount,
}

pub async fn get_hodl_invoice_from_coordinator(amount: Amount) -> Result<HodlInvoice> {
    let pre_image = commons::create_pre_image();

    let client = reqwest_client();
    let url = format!("http://{}", config::get_http_endpoint());
    let url = Url::parse(&url).expect("correct URL");
    let url = url.join("/api/invoice")?;

    let invoice_params = commons::HodlInvoiceParams {
        trader_pubkey: get_node_pubkey(),
        amt_sats: amount.to_sat(),
        r_hash: pre_image.hash.clone(),
    };
    let invoice_params = commons::SignedValue::new(invoice_params, get_node_key())?;

    let response = client
        .post(url)
        .json(&invoice_params)
        .send()
        .await?
        .error_for_status()?;

    let payment_request = response.json::<String>().await?;
    let hodl_invoice = HodlInvoice {
        payment_request,
        pre_image: pre_image.get_base64_encoded_pre_image(),
        r_hash: pre_image.hash,
        amt_sats: Default::default(),
    };
    Ok(hodl_invoice)
}
