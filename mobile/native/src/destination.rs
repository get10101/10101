use crate::api::Destination;
use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::address::NetworkUnchecked;
use bitcoin::Address;
use bitcoin::Amount;
use bitcoin::Network;
use std::str::FromStr;

pub fn decode_destination(destination: String) -> Result<Destination> {
    let node = crate::state::get_node();
    let network = node.inner.network;

    decode_bip21(&destination, network)
        .or(decode_address(destination))
        .context("Failed to parse destination as Bolt11 invoice, Bip21 URI, or on chain address")
}

fn decode_bip21(request: &str, network: Network) -> Result<Destination> {
    let uri: bip21::Uri<'_, NetworkUnchecked, bip21::NoExtras> = request
        .try_into()
        .map_err(|_| anyhow!("request is not valid BIP-21 URI"))?;

    let uri = uri
        .require_network(network)
        .map_err(|e| anyhow!("Invalid network: {e:?}"))?;

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
    ensure!(
        Address::from_str(&request).is_ok(),
        "request is not valid on-chain address"
    );
    Ok(Destination::OnChainAddress(request))
}
