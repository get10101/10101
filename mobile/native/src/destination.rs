use crate::api::Destination;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Address;
use bitcoin::Amount;
use lightning_invoice::Bolt11Invoice;
use lightning_invoice::Bolt11InvoiceDescription;
use std::ops::Add;
use std::str::FromStr;
use std::time::Duration;
use std::time::SystemTime;

pub fn decode_destination(destination: String) -> Result<Destination> {
    decode_bip21(&destination)
        .or(decode_invoice(&destination))
        .or(decode_address(destination))
        .context("Failed to parse destination as Bolt11 invoice, Bip21 URI, or on chain address")
}

fn decode_bip21(request: &str) -> Result<Destination> {
    let uri: bip21::Uri<'_, bip21::NoExtras> = request
        .try_into()
        .map_err(|_| anyhow!("request is not valid BIP-21 URI"))?;
    Ok(Destination::Bip21 {
        address: uri.address.to_string(),
        label: uri
            .label
            .and_then(|l| l.try_into().ok())
            .unwrap_or_default(),
        message: uri
            .message
            .and_then(|m| m.try_into().ok())
            .unwrap_or_default(),
        amount_sats: uri.amount.map(Amount::to_sat),
    })
}

fn decode_address(request: String) -> Result<Destination> {
    anyhow::ensure!(
        Address::from_str(&request).is_ok(),
        "request is not valid on-chain address"
    );
    Ok(Destination::OnChainAddress(request))
}

fn decode_invoice(request: &str) -> Result<Destination> {
    let invoice =
        &Bolt11Invoice::from_str(request).context("request is not valid BOLT11 invoice")?;
    let description = match invoice.description() {
        Bolt11InvoiceDescription::Direct(direct) => direct.to_string(),
        Bolt11InvoiceDescription::Hash(_) => "".to_string(),
    };

    let timestamp = invoice.timestamp();

    let expiry = timestamp
        .add(Duration::from_secs(invoice.expiry_time().as_secs()))
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();

    let timestamp = timestamp.duration_since(SystemTime::UNIX_EPOCH)?.as_secs();

    let payee = match invoice.payee_pub_key() {
        Some(pubkey) => pubkey.to_string(),
        None => invoice.recover_payee_pub_key().to_string(),
    };

    let amount_sats = (invoice.amount_milli_satoshis().unwrap_or(0) as f64 / 1000.0) as u64;

    Ok(Destination::Bolt11 {
        description,
        timestamp,
        expiry,
        amount_sats,
        payee,
    })
}
