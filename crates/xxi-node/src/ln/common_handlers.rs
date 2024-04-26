use crate::bitcoin_conversion::to_script_29;
use crate::bitcoin_conversion::to_tx_30;
use crate::node::Node;
use crate::node::Storage;
use crate::on_chain_wallet::BdkStorage;
use crate::storage::TenTenOneStorage;
use anyhow::Result;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::sign::SpendableOutputDescriptor;
use secp256k1_zkp::Secp256k1;
use std::sync::Arc;

pub fn handle_spendable_outputs<D: BdkStorage, S: TenTenOneStorage, N: Storage>(
    node: &Arc<Node<D, S, N>>,
    outputs: Vec<SpendableOutputDescriptor>,
) -> Result<()> {
    let ldk_outputs = outputs
        .iter()
        .filter(|output| {
            // `StaticOutput`s are sent to the node's on-chain wallet directly
            !matches!(output, SpendableOutputDescriptor::StaticOutput { .. })
        })
        .collect::<Vec<_>>();
    if ldk_outputs.is_empty() {
        return Ok(());
    }
    for spendable_output in ldk_outputs.iter() {
        if let Err(e) = node
            .node_storage
            .insert_spendable_output((*spendable_output).clone())
        {
            tracing::error!("Failed to persist spendable output: {e:#}")
        }
    }
    let destination_script = node.wallet.get_new_address()?;
    let tx_feerate = node
        .fee_rate_estimator
        .get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
    let spending_tx = node.keys_manager.spend_spendable_outputs(
        &ldk_outputs,
        vec![],
        to_script_29(destination_script.script_pubkey()),
        tx_feerate,
        &Secp256k1::new(),
    )?;

    node.blockchain
        .broadcast_transaction_blocking(&to_tx_30(spending_tx))?;

    Ok(())
}
