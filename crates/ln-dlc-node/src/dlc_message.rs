use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
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
        hasher.finalize_fixed().to_hex()
    }
}

impl TryFrom<&SerializedDlcMessage> for Message {
    type Error = anyhow::Error;

    fn try_from(serialized_msg: &SerializedDlcMessage) -> Result<Self, Self::Error> {
        let message: ChannelMessage = serde_json::from_str(&serialized_msg.message)?;
        Ok(Message::Channel(message))
    }
}

impl TryFrom<&Message> for SerializedDlcMessage {
    type Error = anyhow::Error;

    fn try_from(msg: &Message) -> Result<Self, Self::Error> {
        let (message, message_type) = match msg {
            Message::Channel(message) => {
                let message_type = DlcMessageType::from(message);
                let message = serde_json::to_string(&message)?;

                (message, message_type)
            }
            _ => unreachable!(),
        };

        Ok(Self {
            message,
            message_type,
        })
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

impl From<&ChannelMessage> for DlcMessageType {
    fn from(value: &ChannelMessage) -> Self {
        match value {
            ChannelMessage::Offer(_) => DlcMessageType::Offer,
            ChannelMessage::Accept(_) => DlcMessageType::Accept,
            ChannelMessage::Sign(_) => DlcMessageType::Sign,
            ChannelMessage::SettleOffer(_) => DlcMessageType::SettleOffer,
            ChannelMessage::SettleAccept(_) => DlcMessageType::SettleAccept,
            ChannelMessage::SettleConfirm(_) => DlcMessageType::SettleConfirm,
            ChannelMessage::SettleFinalize(_) => DlcMessageType::SettleFinalize,
            ChannelMessage::RenewOffer(_) => DlcMessageType::RenewOffer,
            ChannelMessage::RenewAccept(_) => DlcMessageType::RenewAccept,
            ChannelMessage::RenewConfirm(_) => DlcMessageType::RenewConfirm,
            ChannelMessage::RenewFinalize(_) => DlcMessageType::RenewFinalize,
            ChannelMessage::RenewRevoke(_) => DlcMessageType::RenewRevoke,
            ChannelMessage::CollaborativeCloseOffer(_) => DlcMessageType::CollaborativeCloseOffer,
            ChannelMessage::Reject(_) => DlcMessageType::Reject,
        }
    }
}
