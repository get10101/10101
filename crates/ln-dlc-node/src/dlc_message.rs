use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use dlc_messages::OnChainMessage;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use time::OffsetDateTime;
use ureq::serde_json;

#[derive(Clone)]
pub struct DlcMessage {
    pub message_hash: u64,
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
    pub fn generate_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Hash, Clone, Debug)]
pub enum DlcMessageType {
    OnChain(DlcMessageSubType),
    Channel(DlcMessageSubType),
}

#[derive(Hash, Clone, Debug)]
pub enum DlcMessageSubType {
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
            DlcMessageType::OnChain(serialized_onchain_message_type) => {
                match serialized_onchain_message_type {
                    DlcMessageSubType::Offer => Message::OnChain(OnChainMessage::Offer(
                        serde_json::from_str(&serialized_msg.message)?,
                    )),
                    DlcMessageSubType::Accept => Message::OnChain(OnChainMessage::Accept(
                        serde_json::from_str(&serialized_msg.message)?,
                    )),
                    DlcMessageSubType::Sign => Message::OnChain(OnChainMessage::Sign(
                        serde_json::from_str(&serialized_msg.message)?,
                    )),
                    _ => unreachable!(),
                }
            }
            DlcMessageType::Channel(serialized_channel_message_type) => {
                match serialized_channel_message_type {
                    DlcMessageSubType::Offer => Message::Channel(ChannelMessage::Offer(
                        serde_json::from_str(&serialized_msg.message)?,
                    )),
                    DlcMessageSubType::Accept => Message::Channel(ChannelMessage::Accept(
                        serde_json::from_str(&serialized_msg.message)?,
                    )),
                    DlcMessageSubType::Sign => Message::Channel(ChannelMessage::Sign(
                        serde_json::from_str(&serialized_msg.message)?,
                    )),
                    DlcMessageSubType::SettleOffer => Message::Channel(
                        ChannelMessage::SettleOffer(serde_json::from_str(&serialized_msg.message)?),
                    ),
                    DlcMessageSubType::SettleAccept => {
                        Message::Channel(ChannelMessage::SettleAccept(serde_json::from_str(
                            &serialized_msg.message,
                        )?))
                    }
                    DlcMessageSubType::SettleConfirm => {
                        Message::Channel(ChannelMessage::SettleConfirm(serde_json::from_str(
                            &serialized_msg.message,
                        )?))
                    }
                    DlcMessageSubType::SettleFinalize => {
                        Message::Channel(ChannelMessage::SettleFinalize(serde_json::from_str(
                            &serialized_msg.message,
                        )?))
                    }
                    DlcMessageSubType::RenewOffer => Message::Channel(ChannelMessage::RenewOffer(
                        serde_json::from_str(&serialized_msg.message)?,
                    )),
                    DlcMessageSubType::RenewAccept => Message::Channel(
                        ChannelMessage::RenewAccept(serde_json::from_str(&serialized_msg.message)?),
                    ),
                    DlcMessageSubType::RenewConfirm => {
                        Message::Channel(ChannelMessage::RenewConfirm(serde_json::from_str(
                            &serialized_msg.message,
                        )?))
                    }
                    DlcMessageSubType::RenewFinalize => {
                        Message::Channel(ChannelMessage::RenewFinalize(serde_json::from_str(
                            &serialized_msg.message,
                        )?))
                    }
                    DlcMessageSubType::RenewRevoke => Message::Channel(
                        ChannelMessage::RenewRevoke(serde_json::from_str(&serialized_msg.message)?),
                    ),
                    DlcMessageSubType::CollaborativeCloseOffer => {
                        Message::Channel(ChannelMessage::CollaborativeCloseOffer(
                            serde_json::from_str(&serialized_msg.message)?,
                        ))
                    }
                    DlcMessageSubType::Reject => Message::Channel(ChannelMessage::Reject(
                        serde_json::from_str(&serialized_msg.message)?,
                    )),
                }
            }
        };

        Ok(message)
    }
}

impl TryFrom<&Message> for SerializedDlcMessage {
    type Error = anyhow::Error;

    fn try_from(msg: &Message) -> Result<Self, Self::Error> {
        let (message, message_type) = match &msg {
            Message::OnChain(message) => match message {
                OnChainMessage::Offer(offer) => (
                    serde_json::to_string(&offer)?,
                    DlcMessageType::OnChain(DlcMessageSubType::Offer),
                ),
                OnChainMessage::Accept(accept) => (
                    serde_json::to_string(&accept)?,
                    DlcMessageType::OnChain(DlcMessageSubType::Accept),
                ),
                OnChainMessage::Sign(sign) => (
                    serde_json::to_string(&sign)?,
                    DlcMessageType::OnChain(DlcMessageSubType::Sign),
                ),
            },
            Message::Channel(message) => match message {
                ChannelMessage::Offer(offer) => (
                    serde_json::to_string(&offer)?,
                    DlcMessageType::Channel(DlcMessageSubType::Offer),
                ),
                ChannelMessage::Accept(accept) => (
                    serde_json::to_string(&accept)?,
                    DlcMessageType::Channel(DlcMessageSubType::Accept),
                ),
                ChannelMessage::Sign(sign) => (
                    serde_json::to_string(&sign)?,
                    DlcMessageType::Channel(DlcMessageSubType::Sign),
                ),
                ChannelMessage::SettleOffer(settle_offer) => (
                    serde_json::to_string(&settle_offer)?,
                    DlcMessageType::Channel(DlcMessageSubType::SettleOffer),
                ),
                ChannelMessage::SettleAccept(settle_accept) => (
                    serde_json::to_string(&settle_accept)?,
                    DlcMessageType::Channel(DlcMessageSubType::SettleAccept),
                ),
                ChannelMessage::SettleConfirm(settle_confirm) => (
                    serde_json::to_string(&settle_confirm)?,
                    DlcMessageType::Channel(DlcMessageSubType::SettleConfirm),
                ),
                ChannelMessage::SettleFinalize(settle_finalize) => (
                    serde_json::to_string(&settle_finalize)?,
                    DlcMessageType::Channel(DlcMessageSubType::SettleFinalize),
                ),
                ChannelMessage::RenewOffer(renew_offer) => (
                    serde_json::to_string(&renew_offer)?,
                    DlcMessageType::Channel(DlcMessageSubType::RenewOffer),
                ),
                ChannelMessage::RenewAccept(renew_accept) => (
                    serde_json::to_string(&renew_accept)?,
                    DlcMessageType::Channel(DlcMessageSubType::RenewAccept),
                ),
                ChannelMessage::RenewConfirm(renew_confirm) => (
                    serde_json::to_string(&renew_confirm)?,
                    DlcMessageType::Channel(DlcMessageSubType::RenewConfirm),
                ),
                ChannelMessage::RenewFinalize(renew_finalize) => (
                    serde_json::to_string(&renew_finalize)?,
                    DlcMessageType::Channel(DlcMessageSubType::RenewFinalize),
                ),
                ChannelMessage::RenewRevoke(renew_revoke) => (
                    serde_json::to_string(&renew_revoke)?,
                    DlcMessageType::Channel(DlcMessageSubType::RenewRevoke),
                ),
                ChannelMessage::CollaborativeCloseOffer(collaborative_close_offer) => (
                    serde_json::to_string(&collaborative_close_offer)?,
                    DlcMessageType::Channel(DlcMessageSubType::CollaborativeCloseOffer),
                ),
                ChannelMessage::Reject(reject) => (
                    serde_json::to_string(&reject)?,
                    DlcMessageType::Channel(DlcMessageSubType::Reject),
                ),
            },
            _ => unreachable!(),
        };

        Ok(Self {
            message,
            message_type,
        })
    }
}
