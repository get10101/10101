use bitcoin::Transaction;
use bitcoin::Txid;
use rust_decimal::Decimal;
use secp256k1::ecdsa::Signature;
use serde::Deserialize;
use serde::Serialize;

/// The information needed for the coordinator to kickstart the collaborative revert protocol.
#[derive(Deserialize, Serialize)]
pub struct CollaborativeRevertCoordinatorRequest {
    /// Channel to collaboratively revert.
    pub channel_id: String,
    /// Fee rate for the collaborative revert transaction.
    pub fee_rate_sats_vb: u64,
    /// Amount to be paid out to the counterparty in sats.
    ///
    /// Note: the tx fee will be subtracted evenly between both parties
    pub counter_payout: u64,
    /// The price at which the position has been closed
    ///
    /// Note: this is just for informative purposes and is not used in any calculations
    pub price: Decimal,
}

/// The information provided by the trader in response to a collaborative revert proposal.
#[derive(Deserialize, Serialize)]
pub struct CollaborativeRevertTraderResponse {
    /// Channel to collaboratively revert.
    pub channel_id: String,
    /// The unsigned collaborative revert transaction.
    pub transaction: Transaction,
    /// The trader's signature on the collaborative revert transaction.
    pub signature: Signature,
}

/// The information needed for the coordinator to kickstart the _legacy_ collaborative revert
/// protocol.
#[derive(Deserialize, Serialize)]
pub struct LegacyCollaborativeRevertCoordinatorRequest {
    /// Channel to collaboratively revert.
    pub channel_id: String,
    /// The TXID of the LN funding transaction.
    pub txid: Txid,
    /// The vout corresponding to the funding TXO.
    pub vout: u32,
    /// How much the coordinator should get out of the collaborative revert transaction, without
    /// considering transaction fees.
    pub coordinator_amount: u64,
    /// Fee rate for the collaborative revert transaction.
    pub fee_rate_sats_vb: u64,
    /// Price at which to settle the DLC channel.
    ///
    /// This price is purely informational for the trader, as the caller provides the
    /// `coordinator_amount` already.
    pub price: Decimal,
}

/// The information provided by the trader in response to a _legacy_ collaborative revert proposal.
#[derive(Deserialize, Serialize)]
pub struct LegacyCollaborativeRevertTraderResponse {
    /// Channel to collaboratively revert.
    pub channel_id: String,
    /// The unsigned collaborative revert transaction.
    pub transaction: Transaction,
    /// The trader's signature on the collaborative revert transaction.
    pub signature: Signature,
}
