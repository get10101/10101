use crate::node::Node;
use crate::orderbook::db;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin_old::secp256k1::SecretKey;
use dlc_manager::Signer;
use dlc_messages::channel::RenewRevoke;
use lightning::ln::chan_utils::build_commitment_secret;
use xxi_node::commons::OrderState;
use xxi_node::message_handler::TenTenOneMessage;
use xxi_node::message_handler::TenTenOneRenewRevoke;
use xxi_node::node::event::NodeEvent;

impl Node {
    pub fn resend_renew_revoke_message_internal(&self, trader: PublicKey) -> Result<()> {
        tracing::warn!("Executing emergency kit! Resending renew revoke message");

        let signed_channel = self.inner.get_signed_channel_by_trader_id(trader)?;

        let per_update_seed_pk = signed_channel.own_per_update_seed;
        let per_update_seed = self
            .inner
            .dlc_wallet
            .get_secret_key_for_pubkey(&per_update_seed_pk)?;
        let prev_per_update_secret = SecretKey::from_slice(&build_commitment_secret(
            per_update_seed.as_ref(),
            signed_channel.update_idx + 1,
        ))?;

        let mut conn = self.pool.clone().get()?;
        // We assume the last taken order to be the relevant order.
        let order = db::orders::get_by_trader_id_and_state(&mut conn, trader, OrderState::Taken)?
            .with_context(|| {
            format!("Couldn't find last order in state taken. trader_id={trader}")
        })?;

        let msg = TenTenOneMessage::RenewRevoke(TenTenOneRenewRevoke {
            order_id: order.id,
            renew_revoke: RenewRevoke {
                channel_id: signed_channel.channel_id,
                per_update_secret: prev_per_update_secret,
                reference_id: signed_channel.reference_id,
            },
        });

        self.inner.event_handler.publish(NodeEvent::SendDlcMessage {
            peer: trader,
            msg: msg.clone(),
        });

        Ok(())
    }
}
