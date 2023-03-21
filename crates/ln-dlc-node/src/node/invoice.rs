use crate::node::Node;
use crate::node::LIQUIDITY_ROUTING_FEE_MILLIONTHS;
use crate::PaymentInfo;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::sha256;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::Recipient;
use lightning::ln::channelmanager::MIN_CLTV_EXPIRY_DELTA;
use lightning::ln::channelmanager::MIN_FINAL_CLTV_EXPIRY;
use lightning::ln::PaymentHash;
use lightning::routing::gossip::RoutingFees;
use lightning::routing::router::RouteHint;
use lightning::routing::router::RouteHintHop;
use lightning_invoice::payment::PaymentError;
use lightning_invoice::Currency;
use lightning_invoice::Invoice;
use lightning_invoice::InvoiceBuilder;
use std::time::Duration;
use std::time::SystemTime;

impl Node {
    pub fn create_invoice(&self, amount_in_sats: u64) -> Result<Invoice> {
        lightning_invoice::utils::create_invoice_from_channelmanager(
            &self.channel_manager,
            self.keys_manager.clone(),
            self.logger.clone(),
            self.get_currency(),
            Some(amount_in_sats * 1000),
            "".to_string(),
            180,
        )
        .map_err(|e| anyhow!(e))
    }

    /// Creates an invoice which is meant to be intercepted
    ///
    /// Doing so we need to pass in `intercepted_channel_id` which needs to be generated by the
    /// intercepting node. This information, in combination with `hop_before_me` is used to add a
    /// routing hint to the invoice. Otherwise the sending node does not know how to pay the
    /// invoice
    pub fn create_interceptable_invoice(
        &self,
        amount_in_sats: Option<u64>,
        intercepted_channel_id: u64,
        hop_before_me: PublicKey,
        invoice_expiry: u32,
        description: String,
    ) -> Result<Invoice> {
        let amount_msat = amount_in_sats.map(|x| x * 1000);
        let (payment_hash, payment_secret) = self
            .channel_manager
            .create_inbound_payment(amount_msat, invoice_expiry)
            .unwrap();
        let node_secret = self.keys_manager.get_node_secret(Recipient::Node).unwrap();
        let invoice_builder = InvoiceBuilder::new(self.get_currency())
            .description(description)
            .payment_hash(sha256::Hash::from_slice(&payment_hash.0)?)
            .payment_secret(payment_secret)
            .timestamp(SystemTime::now())
            // lnd defaults the min final cltv to 9 (according to BOLT 11 - the recommendation has
            // changed to 18) 9 is not safe to use for ldk, because ldk mandates that
            // the `cltv_expiry_delta` has to be greater than `HTLC_FAIL_BACK_BUFFER`
            // (23).
            .min_final_cltv_expiry(MIN_FINAL_CLTV_EXPIRY as u64)
            .private_route(RouteHint(vec![RouteHintHop {
                src_node_id: hop_before_me,
                short_channel_id: intercepted_channel_id,
                // QUESTION: What happens if these differ with the actual values
                // in the `ChannelConfig` for the private channel?
                fees: RoutingFees {
                    base_msat: 1000,
                    proportional_millionths: LIQUIDITY_ROUTING_FEE_MILLIONTHS,
                },
                cltv_expiry_delta: MIN_CLTV_EXPIRY_DELTA,
                htlc_minimum_msat: None,
                htlc_maximum_msat: None,
            }]));

        let invoice_builder = match amount_msat {
            Some(msats) => invoice_builder.amount_milli_satoshis(msats),
            None => invoice_builder,
        };

        let signed_invoice = invoice_builder
            .build_raw()
            .unwrap()
            .sign::<_, ()>(|hash| {
                let secp_ctx = Secp256k1::new();
                Ok(secp_ctx.sign_ecdsa_recoverable(hash, &node_secret))
            })
            .unwrap();
        let invoice = Invoice::from_signed(signed_invoice).unwrap();
        Ok(invoice)
    }

    fn get_currency(&self) -> Currency {
        match self.network {
            Network::Bitcoin => Currency::Bitcoin,
            Network::Testnet => Currency::BitcoinTestnet,
            Network::Regtest => Currency::Regtest,
            Network::Signet => Currency::Signet,
        }
    }

    /// Creates a fake channel id needed to intercept payments to the provided `target_node`
    ///
    /// This is mainly used for instant payments where the receiver does not have a lightning
    /// channel yet, e.g. Alice does not have a channel with Bob yet but wants to
    /// receive a LN payment. Clair pays to Bob who opens a channel to Alice and pays her.
    pub fn create_intercept_scid(&self, target_node: PublicKey) -> u64 {
        let intercept_scid = self.channel_manager.get_intercept_scid();
        self.fake_channel_payments
            .lock()
            .unwrap()
            .insert(intercept_scid, target_node);
        intercept_scid
    }

    pub fn send_payment(&self, invoice: &Invoice) -> Result<()> {
        match self.invoice_payer.pay_invoice(invoice) {
            Ok(_payment_id) => {
                let payee_pubkey = invoice.recover_payee_pub_key();
                let amt_msat = invoice
                    .amount_milli_satoshis()
                    .context("invalid msat amount in the invoice")?;
                tracing::info!("EVENT: initiated sending {amt_msat} msats to {payee_pubkey}",);
                HTLCStatus::Pending
            }
            Err(PaymentError::Invoice(err)) => {
                tracing::error!(%err, "Invalid invoice");
                anyhow::bail!(err);
            }
            Err(PaymentError::Routing(err)) => {
                tracing::error!(?err, "Failed to find route");
                anyhow::bail!("{:?}", err);
            }
            Err(PaymentError::Sending(err)) => {
                tracing::error!(?err, "Failed to send payment");
                HTLCStatus::Failed
            }
        };
        Ok(())
    }

    pub async fn wait_for_payment_claimed(
        &self,
        hash: &sha256::Hash,
    ) -> Result<(), tokio::time::error::Elapsed> {
        let payment_hash = PaymentHash(hash.into_inner());

        tokio::time::timeout(Duration::from_secs(6), async {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;

                match self.inbound_payments.lock().unwrap().get(&payment_hash) {
                    Some(PaymentInfo {
                        status: HTLCStatus::Succeeded,
                        ..
                    }) => return,
                    Some(PaymentInfo { status, .. }) => {
                        tracing::debug!(
                            payment_hash = %hex::encode(hash),
                            ?status,
                            "Checking if payment has been claimed"
                        );
                    }
                    None => {
                        tracing::debug!(
                            payment_hash = %hex::encode(hash),
                            status = "unknown",
                            "Checking if payment has been claimed"
                        );
                    }
                }
            }
        })
        .await
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HTLCStatus {
    Pending,
    Succeeded,
    Failed,
}
