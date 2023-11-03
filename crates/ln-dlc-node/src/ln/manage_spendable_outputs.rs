use crate::dlc_custom_signer::CustomKeysManager;
use crate::fee_rate_estimator::FeeRateEstimator;
use crate::ln_dlc_wallet::LnDlcWallet;
use crate::node::Storage;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use esplora_client::OutputStatus;
use esplora_client::TxStatus;
use lightning::chain::chaininterface::BroadcasterInterface;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::chain::transaction::OutPoint;
use lightning::sign::DelayedPaymentOutputDescriptor;
use lightning::sign::SpendableOutputDescriptor;
use lightning::sign::StaticPaymentOutputDescriptor;
use secp256k1_zkp::Secp256k1;
use std::borrow::Borrow;
use std::sync::Arc;

/// Number of confirmations required to consider an LDK spendable output _spent_.
///
/// We use this to determine which outputs we can forget about.
const REQUIRED_CONFIRMATIONS: u32 = 6;

/// Determine what to do with a [`SpendableOutputDescriptor`] and do it.
pub fn manage_spendable_outputs(
    node_storage: Arc<impl Storage>,
    esplora_client: impl Borrow<esplora_client::BlockingClient>,
    wallet: impl Borrow<LnDlcWallet>,
    fee_rate_estimator: impl Borrow<FeeRateEstimator>,
    keys_manager: impl Borrow<CustomKeysManager>,
) -> Result<()> {
    let mut outputs_to_spend = Vec::new();

    let spendable_outputs = &node_storage.all_spendable_outputs()?;
    for output in spendable_outputs.iter() {
        let action = match choose_spendable_output_action(esplora_client.borrow(), output) {
            Ok(action) => action,
            Err(e) => {
                tracing::error!(
                    ?output,
                    "Failed to choose action to take for spendable output: {e:#}"
                );
                continue;
            }
        };

        match action {
            Action::Spend => outputs_to_spend.push(output),
            Action::Forget(outpoint) => {
                tracing::debug!(?output, "Deleting output from storage");
                if let Err(e) = node_storage.delete_spendable_output(&outpoint) {
                    tracing::error!("Failed to delete forgettable spendable output: {e:#}");
                }
            }
            Action::Monitor => continue,
        }
    }

    if spendable_outputs.is_empty() {
        return Ok(());
    }

    let destination_script = wallet.borrow().ldk_wallet().get_last_unused_address()?;
    let tx_feerate = fee_rate_estimator
        .borrow()
        .get_est_sat_per_1000_weight(ConfirmationTarget::Normal);

    let spending_tx = keys_manager.borrow().spend_spendable_outputs(
        outputs_to_spend.as_slice(),
        vec![],
        destination_script.script_pubkey(),
        tx_feerate,
        &Secp256k1::new(),
    )?;
    wallet.borrow().broadcast_transactions(&[&spending_tx]);

    Ok(())
}

enum Action {
    Spend,
    Monitor,
    Forget(OutPoint),
}

/// Decide on which [`Action`] should be performed based on the characteristics and status of a
/// [`SpendableOutputDescriptor`].
fn choose_spendable_output_action(
    esplora_client: &esplora_client::BlockingClient,
    output: &SpendableOutputDescriptor,
) -> Result<Action> {
    let esplora_client = esplora_client.borrow();

    use SpendableOutputDescriptor::*;
    let outpoint = match output {
        StaticPaymentOutput(StaticPaymentOutputDescriptor { outpoint, .. })
        | DelayedPaymentOutput(DelayedPaymentOutputDescriptor { outpoint, .. }) => outpoint,
        // These are already owned by our on-chain wallet.
        StaticOutput { outpoint, .. } => return Ok(Action::Forget(*outpoint)),
    };

    let output_status = esplora_client
        .get_output_status(&outpoint.txid, outpoint.index.into())
        .context("Could not get spendable output status")?;

    match output_status {
        Some(OutputStatus { spent: false, .. }) | None => {
            tracing::debug!(?output, "Spendable output not yet spent");
            Ok(Action::Spend)
        }
        Some(OutputStatus {
            status:
                Some(TxStatus {
                    confirmed: true,
                    block_height: Some(confirmation_height),
                    ..
                }),
            ..
        }) => {
            let current_height = esplora_client.get_height()?;

            let confirmations = current_height
                .checked_sub(confirmation_height)
                .unwrap_or_else(|| {
                    tracing::warn!(
                        %confirmation_height,
                        %current_height,
                        "Possible re-org detected"
                    );

                    0
                });

            if confirmations >= REQUIRED_CONFIRMATIONS {
                tracing::info!(
                    %confirmations,
                    required_confirmations = %REQUIRED_CONFIRMATIONS,
                    "Spendable output sufficiently confirmed"
                );

                Ok(Action::Forget(*outpoint))
            } else {
                tracing::info!(
                    %confirmations,
                    required_confirmations = %REQUIRED_CONFIRMATIONS,
                    "Spendable output without enough confirmations"
                );

                Ok(Action::Monitor)
            }
        }
        Some(_) => {
            bail!("Spendable output in unexpected state: {output:?}");
        }
    }
}
