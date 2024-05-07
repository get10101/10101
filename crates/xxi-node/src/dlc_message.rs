use crate::message_handler::TenTenOneMessage;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use sha2::digest::FixedOutput;
use sha2::Digest;
use sha2::Sha256;
use time::OffsetDateTime;
use ureq::serde_json;

#[derive(Clone)]
pub struct DlcMessage {
    pub message_hash: String,
    pub inbound: bool,
    pub peer_id: PublicKey,
    pub message_type: DlcMessageType,
    pub timestamp: OffsetDateTime,
}

impl DlcMessage {
    pub fn new(
        peer_id: PublicKey,
        serialized_message: SerializedDlcMessage,
        inbound: bool,
    ) -> Result<DlcMessage> {
        let message_hash = serialized_message.generate_hash();

        Ok(Self {
            message_hash,
            inbound,
            peer_id,
            message_type: serialized_message.message_type,
            timestamp: OffsetDateTime::now_utc(),
        })
    }
}

#[derive(Hash, Clone, Debug)]
pub struct SerializedDlcMessage {
    pub message: String,
    pub message_type: DlcMessageType,
}

impl SerializedDlcMessage {
    pub fn generate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.message.as_bytes());
        hex::encode(hasher.finalize_fixed())
    }
}

#[derive(Hash, Clone, Debug)]
pub enum DlcMessageType {
    Offer,
    Accept,
    Sign,
    SettleOffer,
    SettleAccept,
    SettleConfirm,
    SettleFinalize,
    RenewOffer,
    RenewAccept,
    RenewConfirm,
    RenewFinalize,
    RenewRevoke,
    RolloverOffer,
    RolloverAccept,
    RolloverConfirm,
    RolloverFinalize,
    RolloverRevoke,
    CollaborativeCloseOffer,
    Reject,
}

impl TryFrom<&SerializedDlcMessage> for TenTenOneMessage {
    type Error = anyhow::Error;

    fn try_from(serialized_msg: &SerializedDlcMessage) -> Result<Self, Self::Error> {
        let message = match serialized_msg.clone().message_type {
            DlcMessageType::Reject => {
                TenTenOneMessage::Reject(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::Offer => {
                TenTenOneMessage::Offer(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::Accept => {
                TenTenOneMessage::Accept(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::Sign => {
                TenTenOneMessage::Sign(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::SettleOffer => {
                TenTenOneMessage::SettleOffer(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::SettleAccept => {
                TenTenOneMessage::SettleAccept(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::SettleConfirm => {
                TenTenOneMessage::SettleConfirm(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::SettleFinalize => {
                TenTenOneMessage::SettleFinalize(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RenewOffer => {
                TenTenOneMessage::RenewOffer(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RenewAccept => {
                TenTenOneMessage::RenewAccept(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RenewConfirm => {
                TenTenOneMessage::RenewConfirm(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RenewFinalize => {
                TenTenOneMessage::RenewFinalize(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RenewRevoke => {
                TenTenOneMessage::RenewRevoke(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RolloverOffer => {
                TenTenOneMessage::RolloverOffer(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RolloverAccept => {
                TenTenOneMessage::RolloverAccept(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RolloverConfirm => {
                TenTenOneMessage::RolloverConfirm(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RolloverFinalize => {
                TenTenOneMessage::RolloverFinalize(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::RolloverRevoke => {
                TenTenOneMessage::RolloverRevoke(serde_json::from_str(&serialized_msg.message)?)
            }
            DlcMessageType::CollaborativeCloseOffer => TenTenOneMessage::CollaborativeCloseOffer(
                serde_json::from_str(&serialized_msg.message)?,
            ),
        };

        Ok(message)
    }
}

impl TryFrom<&TenTenOneMessage> for SerializedDlcMessage {
    type Error = anyhow::Error;

    fn try_from(msg: &TenTenOneMessage) -> Result<Self, Self::Error> {
        let (message, message_type) = match &msg {
            TenTenOneMessage::Offer(offer) => {
                (serde_json::to_string(&offer)?, DlcMessageType::Offer)
            }
            TenTenOneMessage::Accept(accept) => {
                (serde_json::to_string(&accept)?, DlcMessageType::Accept)
            }
            TenTenOneMessage::Sign(sign) => (serde_json::to_string(&sign)?, DlcMessageType::Sign),
            TenTenOneMessage::SettleOffer(settle_offer) => (
                serde_json::to_string(&settle_offer)?,
                DlcMessageType::SettleOffer,
            ),
            TenTenOneMessage::SettleAccept(settle_accept) => (
                serde_json::to_string(&settle_accept)?,
                DlcMessageType::SettleAccept,
            ),
            TenTenOneMessage::SettleConfirm(settle_confirm) => (
                serde_json::to_string(&settle_confirm)?,
                DlcMessageType::SettleConfirm,
            ),
            TenTenOneMessage::SettleFinalize(settle_finalize) => (
                serde_json::to_string(&settle_finalize)?,
                DlcMessageType::SettleFinalize,
            ),
            TenTenOneMessage::RenewOffer(renew_offer) => (
                serde_json::to_string(&renew_offer)?,
                DlcMessageType::RenewOffer,
            ),
            TenTenOneMessage::RenewAccept(renew_accept) => (
                serde_json::to_string(&renew_accept)?,
                DlcMessageType::RenewAccept,
            ),
            TenTenOneMessage::RenewConfirm(renew_confirm) => (
                serde_json::to_string(&renew_confirm)?,
                DlcMessageType::RenewConfirm,
            ),
            TenTenOneMessage::RenewFinalize(renew_finalize) => (
                serde_json::to_string(&renew_finalize)?,
                DlcMessageType::RenewFinalize,
            ),
            TenTenOneMessage::RenewRevoke(renew_revoke) => (
                serde_json::to_string(&renew_revoke)?,
                DlcMessageType::RenewRevoke,
            ),
            TenTenOneMessage::CollaborativeCloseOffer(collaborative_close_offer) => (
                serde_json::to_string(&collaborative_close_offer)?,
                DlcMessageType::CollaborativeCloseOffer,
            ),
            TenTenOneMessage::Reject(reject) => {
                (serde_json::to_string(&reject)?, DlcMessageType::Reject)
            }
            TenTenOneMessage::RolloverOffer(rollover_offer) => (
                serde_json::to_string(&rollover_offer)?,
                DlcMessageType::RolloverOffer,
            ),
            TenTenOneMessage::RolloverAccept(rollover_accept) => (
                serde_json::to_string(&rollover_accept)?,
                DlcMessageType::RolloverAccept,
            ),
            TenTenOneMessage::RolloverConfirm(rollover_confirm) => (
                serde_json::to_string(&rollover_confirm)?,
                DlcMessageType::RolloverConfirm,
            ),
            TenTenOneMessage::RolloverFinalize(rollover_finalize) => (
                serde_json::to_string(&rollover_finalize)?,
                DlcMessageType::RolloverFinalize,
            ),
            TenTenOneMessage::RolloverRevoke(rollover_revoke) => (
                serde_json::to_string(&rollover_revoke)?,
                DlcMessageType::RolloverRevoke,
            ),
        };

        Ok(Self {
            message,
            message_type,
        })
    }
}
