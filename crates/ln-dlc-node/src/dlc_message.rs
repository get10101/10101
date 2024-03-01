use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
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
    CollaborativeCloseOffer,
    Reject,
}

impl TryFrom<&SerializedDlcMessage> for Message {
    type Error = anyhow::Error;

    fn try_from(serialized_msg: &SerializedDlcMessage) -> Result<Self, Self::Error> {
        let message = match serialized_msg.clone().message_type {
            DlcMessageType::Offer => Message::Channel(ChannelMessage::Offer(serde_json::from_str(
                &serialized_msg.message,
            )?)),
            DlcMessageType::Accept => Message::Channel(ChannelMessage::Accept(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::Sign => Message::Channel(ChannelMessage::Sign(serde_json::from_str(
                &serialized_msg.message,
            )?)),
            DlcMessageType::SettleOffer => Message::Channel(ChannelMessage::SettleOffer(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::SettleAccept => Message::Channel(ChannelMessage::SettleAccept(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::SettleConfirm => Message::Channel(ChannelMessage::SettleConfirm(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::SettleFinalize => Message::Channel(ChannelMessage::SettleFinalize(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::RenewOffer => Message::Channel(ChannelMessage::RenewOffer(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::RenewAccept => Message::Channel(ChannelMessage::RenewAccept(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::RenewConfirm => Message::Channel(ChannelMessage::RenewConfirm(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::RenewFinalize => Message::Channel(ChannelMessage::RenewFinalize(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::RenewRevoke => Message::Channel(ChannelMessage::RenewRevoke(
                serde_json::from_str(&serialized_msg.message)?,
            )),
            DlcMessageType::CollaborativeCloseOffer => {
                Message::Channel(ChannelMessage::CollaborativeCloseOffer(
                    serde_json::from_str(&serialized_msg.message)?,
                ))
            }
            DlcMessageType::Reject => Message::Channel(ChannelMessage::Reject(
                serde_json::from_str(&serialized_msg.message)?,
            )),
        };

        Ok(message)
    }
}

impl TryFrom<&Message> for SerializedDlcMessage {
    type Error = anyhow::Error;

    fn try_from(msg: &Message) -> Result<Self, Self::Error> {
        let (message, message_type) = match &msg {
            Message::Channel(message) => match message {
                ChannelMessage::Offer(offer) => {
                    (serde_json::to_string(&offer)?, DlcMessageType::Offer)
                }
                ChannelMessage::Accept(accept) => {
                    (serde_json::to_string(&accept)?, DlcMessageType::Accept)
                }
                ChannelMessage::Sign(sign) => (serde_json::to_string(&sign)?, DlcMessageType::Sign),
                ChannelMessage::SettleOffer(settle_offer) => (
                    serde_json::to_string(&settle_offer)?,
                    DlcMessageType::SettleOffer,
                ),
                ChannelMessage::SettleAccept(settle_accept) => (
                    serde_json::to_string(&settle_accept)?,
                    DlcMessageType::SettleAccept,
                ),
                ChannelMessage::SettleConfirm(settle_confirm) => (
                    serde_json::to_string(&settle_confirm)?,
                    DlcMessageType::SettleConfirm,
                ),
                ChannelMessage::SettleFinalize(settle_finalize) => (
                    serde_json::to_string(&settle_finalize)?,
                    DlcMessageType::SettleFinalize,
                ),
                ChannelMessage::RenewOffer(renew_offer) => (
                    serde_json::to_string(&renew_offer)?,
                    DlcMessageType::RenewOffer,
                ),
                ChannelMessage::RenewAccept(renew_accept) => (
                    serde_json::to_string(&renew_accept)?,
                    DlcMessageType::RenewAccept,
                ),
                ChannelMessage::RenewConfirm(renew_confirm) => (
                    serde_json::to_string(&renew_confirm)?,
                    DlcMessageType::RenewConfirm,
                ),
                ChannelMessage::RenewFinalize(renew_finalize) => (
                    serde_json::to_string(&renew_finalize)?,
                    DlcMessageType::RenewFinalize,
                ),
                ChannelMessage::RenewRevoke(renew_revoke) => (
                    serde_json::to_string(&renew_revoke)?,
                    DlcMessageType::RenewRevoke,
                ),
                ChannelMessage::CollaborativeCloseOffer(collaborative_close_offer) => (
                    serde_json::to_string(&collaborative_close_offer)?,
                    DlcMessageType::CollaborativeCloseOffer,
                ),
                ChannelMessage::Reject(reject) => {
                    (serde_json::to_string(&reject)?, DlcMessageType::Reject)
                }
            },
            _ => unreachable!(),
        };

        Ok(Self {
            message,
            message_type,
        })
    }
}
