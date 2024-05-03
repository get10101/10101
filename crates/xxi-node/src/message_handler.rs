use crate::bitcoin_conversion::to_secp_pk_30;
use crate::commons::FilledWith;
use crate::commons::Order;
use crate::node::event::NodeEvent;
use crate::node::event::NodeEventHandler;
use anyhow::Result;
use dlc_manager::ReferenceId;
use dlc_messages::channel::AcceptChannel;
use dlc_messages::channel::CollaborativeCloseOffer;
use dlc_messages::channel::OfferChannel;
use dlc_messages::channel::Reject;
use dlc_messages::channel::RenewAccept;
use dlc_messages::channel::RenewConfirm;
use dlc_messages::channel::RenewFinalize;
use dlc_messages::channel::RenewOffer;
use dlc_messages::channel::RenewRevoke;
use dlc_messages::channel::SettleAccept;
use dlc_messages::channel::SettleConfirm;
use dlc_messages::channel::SettleFinalize;
use dlc_messages::channel::SettleOffer;
use dlc_messages::channel::SignChannel;
use dlc_messages::field_read;
use dlc_messages::field_write;
use dlc_messages::impl_dlc_writeable;
use dlc_messages::segmentation;
use dlc_messages::segmentation::get_segments;
use dlc_messages::segmentation::segment_reader::SegmentReader;
use dlc_messages::segmentation::SegmentChunk;
use dlc_messages::segmentation::SegmentStart;
use dlc_messages::ser_impls::read_string;
use dlc_messages::ser_impls::write_string;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use lightning::events::OnionMessageProvider;
use lightning::ln::features::InitFeatures;
use lightning::ln::features::NodeFeatures;
use lightning::ln::msgs;
use lightning::ln::msgs::DecodeError;
use lightning::ln::msgs::LightningError;
use lightning::ln::msgs::OnionMessage;
use lightning::ln::msgs::OnionMessageHandler;
use lightning::ln::peer_handler::CustomMessageHandler;
use lightning::ln::wire::CustomMessageReader;
use lightning::ln::wire::Type;
use lightning::util::ser::Readable;
use lightning::util::ser::Writeable;
use lightning::util::ser::Writer;
use lightning::util::ser::MAX_BUF_SIZE;
use secp256k1_zkp::PublicKey;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt::Display;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;

/// TenTenOneMessageHandler is used to send and receive messages through the custom
/// message handling mechanism of the LDK. It also handles message segmentation
/// by splitting large messages when sending and re-constructing them when
/// receiving.
pub struct TenTenOneMessageHandler {
    handler: Arc<NodeEventHandler>,
    msg_events: Mutex<VecDeque<(PublicKey, WireMessage)>>,
    msg_received: Mutex<Vec<(PublicKey, TenTenOneMessage)>>,
    segment_readers: Mutex<HashMap<PublicKey, SegmentReader>>,
}

impl TenTenOneMessageHandler {
    pub fn new(handler: Arc<NodeEventHandler>) -> Self {
        Self {
            handler,
            msg_events: Mutex::new(Default::default()),
            msg_received: Mutex::new(vec![]),
            segment_readers: Mutex::new(Default::default()),
        }
    }
}

/// Copied from the IgnoringMessageHandler
impl OnionMessageProvider for TenTenOneMessageHandler {
    fn next_onion_message_for_peer(&self, _peer_node_id: PublicKey) -> Option<OnionMessage> {
        None
    }
}

/// Copied primarily from the IgnoringMessageHandler. Using the peer_connected hook to get notified
/// once a peer successfully connected. (This also includes that the Init Message has been processed
/// and the connection is ready to use).
impl OnionMessageHandler for TenTenOneMessageHandler {
    fn handle_onion_message(&self, _their_node_id: &PublicKey, _msg: &OnionMessage) {}
    fn peer_connected(
        &self,
        their_node_id: &PublicKey,
        _init: &msgs::Init,
        inbound: bool,
    ) -> Result<(), ()> {
        tracing::info!(%their_node_id, inbound, "Peer connected!");

        self.handler.publish(NodeEvent::Connected {
            peer: to_secp_pk_30(*their_node_id),
        });

        Ok(())
    }
    fn peer_disconnected(&self, _their_node_id: &PublicKey) {}
    fn provided_node_features(&self) -> NodeFeatures {
        NodeFeatures::empty()
    }
    fn provided_init_features(&self, _their_node_id: &PublicKey) -> InitFeatures {
        InitFeatures::empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMessage {
    Message(TenTenOneMessage),
    SegmentStart(SegmentStart),
    SegmentChunk(SegmentChunk),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum TenTenOneMessage {
    Reject(TenTenOneReject),
    Offer(TenTenOneOfferChannel),
    Accept(TenTenOneAcceptChannel),
    Sign(TenTenOneSignChannel),
    SettleOffer(TenTenOneSettleOffer),
    SettleAccept(TenTenOneSettleAccept),
    SettleConfirm(TenTenOneSettleConfirm),
    SettleFinalize(TenTenOneSettleFinalize),
    RenewOffer(TenTenOneRenewOffer),
    RolloverOffer(TenTenOneRolloverOffer),
    RenewAccept(TenTenOneRenewAccept),
    RenewConfirm(TenTenOneRenewConfirm),
    RenewFinalize(TenTenOneRenewFinalize),
    RenewRevoke(TenTenOneRenewRevoke),
    CollaborativeCloseOffer(TenTenOneCollaborativeCloseOffer),
}

impl TenTenOneMessage {
    /// Returns true if the message is a trade message. e.g. Reject, Rollover or Collaborative Close
    /// Offer are not trade messages.
    ///
    /// FIXME(holzeis): This will also return true for any message past the offe rin case of a
    /// rollover. We proobably need to add some hint on the message that this is due to a rollover.
    pub fn is_trade(&self) -> bool {
        matches!(
            self,
            TenTenOneMessage::Offer(_)
                | TenTenOneMessage::Accept(_)
                | TenTenOneMessage::Sign(_)
                | TenTenOneMessage::RenewOffer(_)
                | TenTenOneMessage::RenewAccept(_)
                | TenTenOneMessage::RenewConfirm(_)
                | TenTenOneMessage::RenewFinalize(_)
                | TenTenOneMessage::RenewRevoke(_)
                | TenTenOneMessage::SettleOffer(_)
                | TenTenOneMessage::SettleAccept(_)
                | TenTenOneMessage::SettleConfirm(_)
                | TenTenOneMessage::SettleFinalize(_)
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneReject {
    pub reject: Reject,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TenTenOneOfferChannel {
    pub filled_with: FilledWith,
    pub offer_channel: OfferChannel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneAcceptChannel {
    pub accept_channel: AcceptChannel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneSignChannel {
    pub sign_channel: SignChannel,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TenTenOneSettleOffer {
    pub order: Order,
    pub filled_with: FilledWith,
    pub settle_offer: SettleOffer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneSettleAccept {
    pub settle_accept: SettleAccept,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneSettleConfirm {
    pub settle_confirm: SettleConfirm,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneSettleFinalize {
    pub settle_finalize: SettleFinalize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TenTenOneRenewOffer {
    pub filled_with: FilledWith,
    pub renew_offer: RenewOffer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TenTenOneRolloverOffer {
    pub renew_offer: RenewOffer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneRenewAccept {
    pub renew_accept: RenewAccept,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneRenewConfirm {
    pub renew_confirm: RenewConfirm,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneRenewFinalize {
    pub renew_finalize: RenewFinalize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneRenewRevoke {
    pub renew_revoke: RenewRevoke,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenTenOneCollaborativeCloseOffer {
    pub collaborative_close_offer: CollaborativeCloseOffer,
}

impl TenTenOneMessageHandler {
    /// Returns whether there are any new received messages to process.
    pub fn has_pending_messages_to_process(&self) -> bool {
        !self.msg_received.lock().expect("to get lock").is_empty()
    }

    /// Returns the messages received by the message handler and empty the
    /// receiving buffer.
    pub fn get_and_clear_received_messages(&self) -> Vec<(PublicKey, TenTenOneMessage)> {
        let mut ret = Vec::new();
        std::mem::swap(
            &mut *self.msg_received.lock().expect("to get lock"),
            &mut ret,
        );
        ret
    }

    /// Send a message to the peer with given node id. Not that the message is not
    /// sent right away, but only when the LDK
    /// [`lightning::ln::peer_handler::PeerManager::process_events`] is next called.
    pub fn send_message(&self, node_id: PublicKey, msg: TenTenOneMessage) {
        if msg.serialized_length() > MAX_BUF_SIZE {
            let (seg_start, seg_chunks) = get_segments(msg.encode(), msg.type_id());
            let mut msg_events = self.msg_events.lock().expect("to get lock");
            msg_events.push_back((node_id, WireMessage::SegmentStart(seg_start)));
            for chunk in seg_chunks {
                msg_events.push_back((node_id, WireMessage::SegmentChunk(chunk)));
            }
        } else {
            self.msg_events
                .lock()
                .expect("to get lock")
                .push_back((node_id, WireMessage::Message(msg)));
        }
    }

    /// Returns whether the message handler has any message to be sent.
    pub fn has_pending_messages(&self) -> bool {
        !self.msg_events.lock().expect("to get lock").is_empty()
    }
}

impl CustomMessageReader for TenTenOneMessageHandler {
    type CustomMessage = WireMessage;
    fn read<R: ::std::io::Read>(
        &self,
        msg_type: u16,
        mut buffer: &mut R,
    ) -> Result<Option<WireMessage>, DecodeError> {
        let decoded = match msg_type {
            segmentation::SEGMENT_START_TYPE => {
                WireMessage::SegmentStart(Readable::read(&mut buffer)?)
            }
            segmentation::SEGMENT_CHUNK_TYPE => {
                WireMessage::SegmentChunk(Readable::read(&mut buffer)?)
            }
            _ => return read_tentenone_message(msg_type, buffer),
        };

        Ok(Some(decoded))
    }
}

/// Implementation of the `CustomMessageHandler` trait is required to handle
/// custom messages in the LDK.
impl CustomMessageHandler for TenTenOneMessageHandler {
    fn handle_custom_message(
        &self,
        msg: WireMessage,
        org: &PublicKey,
    ) -> Result<(), LightningError> {
        let mut segment_readers = self.segment_readers.lock().expect("to get lock");
        let segment_reader = segment_readers.entry(*org).or_default();

        if segment_reader.expecting_chunk() {
            match msg {
                WireMessage::SegmentChunk(s) => {
                    if let Some(msg) = segment_reader
                        .process_segment_chunk(s)
                        .map_err(|e| to_ln_error(e, "Error processing segment chunk"))?
                    {
                        let mut buf = Cursor::new(msg);
                        let message_type = <u16 as Readable>::read(&mut buf).map_err(|e| {
                            to_ln_error(e, "Could not reconstruct message from segments")
                        })?;
                        if let WireMessage::Message(m) = self
                            .read(message_type, &mut buf)
                            .map_err(|e| {
                                to_ln_error(e, "Could not reconstruct message from segments")
                            })?
                            .expect("to have a message")
                        {
                            self.msg_received
                                .lock()
                                .expect("to get lock")
                                .push((*org, m));
                        } else {
                            return Err(to_ln_error(
                                "Unexpected message type",
                                &message_type.to_string(),
                            ));
                        }
                    }
                    return Ok(());
                }
                _ => {
                    // We were expecting a segment chunk but received something
                    // else, we reset the state.
                    segment_reader.reset();
                }
            }
        }

        match msg {
            WireMessage::Message(m) => self
                .msg_received
                .lock()
                .expect("to get lock")
                .push((*org, m)),
            WireMessage::SegmentStart(s) => segment_reader
                .process_segment_start(s)
                .map_err(|e| to_ln_error(e, "Error processing segment start"))?,
            WireMessage::SegmentChunk(_) => {
                return Err(LightningError {
                    err: "Received a SegmentChunk while not expecting one.".to_string(),
                    action: lightning::ln::msgs::ErrorAction::DisconnectPeer { msg: None },
                });
            }
        };
        Ok(())
    }

    fn get_and_clear_pending_msg(&self) -> Vec<(PublicKey, Self::CustomMessage)> {
        self.msg_events
            .lock()
            .expect("to get lock")
            .drain(..)
            .collect()
    }

    fn provided_node_features(&self) -> NodeFeatures {
        NodeFeatures::empty()
    }

    fn provided_init_features(&self, _their_node_id: &PublicKey) -> InitFeatures {
        InitFeatures::empty()
    }
}

#[inline]
fn to_ln_error<T: Display>(e: T, msg: &str) -> LightningError {
    LightningError {
        err: format!("{} :{}", msg, e),
        action: lightning::ln::msgs::ErrorAction::DisconnectPeer { msg: None },
    }
}

pub fn tentenone_message_name(msg: &TenTenOneMessage) -> String {
    let name = match msg {
        TenTenOneMessage::Offer(_) => "Offer",
        TenTenOneMessage::Accept(_) => "Accept",
        TenTenOneMessage::Sign(_) => "Sign",
        TenTenOneMessage::SettleOffer(_) => "SettleOffer",
        TenTenOneMessage::SettleAccept(_) => "SettleAccept",
        TenTenOneMessage::SettleConfirm(_) => "SettleConfirm",
        TenTenOneMessage::SettleFinalize(_) => "SettleFinalize",
        TenTenOneMessage::RenewOffer(_) => "RenewOffer",
        TenTenOneMessage::RolloverOffer(_) => "RolloverOffer",
        TenTenOneMessage::RenewAccept(_) => "RenewAccept",
        TenTenOneMessage::RenewConfirm(_) => "RenewConfirm",
        TenTenOneMessage::RenewFinalize(_) => "RenewFinalize",
        TenTenOneMessage::RenewRevoke(_) => "RenewRevoke",
        TenTenOneMessage::CollaborativeCloseOffer(_) => "CollaborativeCloseOffer",
        TenTenOneMessage::Reject(_) => "Reject",
    };

    name.to_string()
}

impl TenTenOneMessage {
    /// Builds a 10101 message from the rust-dlc response message. Note, a response can never return
    /// an offer so if an offer is passed the function will panic. This is most likely not a future
    /// proof solution as we'd might want to enrich the response with 10101 metadata as well. If
    /// that happens we will have to rework this part.
    pub fn build_from_response(message: Message) -> Self {
        match message {
            Message::Channel(ChannelMessage::Accept(accept_channel)) => {
                TenTenOneMessage::Accept(TenTenOneAcceptChannel { accept_channel })
            }
            Message::Channel(ChannelMessage::Sign(sign_channel)) => {
                TenTenOneMessage::Sign(TenTenOneSignChannel { sign_channel })
            }
            Message::Channel(ChannelMessage::SettleAccept(settle_accept)) => {
                TenTenOneMessage::SettleAccept(TenTenOneSettleAccept { settle_accept })
            }
            Message::Channel(ChannelMessage::SettleConfirm(settle_confirm)) => {
                TenTenOneMessage::SettleConfirm(TenTenOneSettleConfirm { settle_confirm })
            }
            Message::Channel(ChannelMessage::SettleFinalize(settle_finalize)) => {
                TenTenOneMessage::SettleFinalize(TenTenOneSettleFinalize { settle_finalize })
            }
            Message::Channel(ChannelMessage::RenewAccept(renew_accept)) => {
                TenTenOneMessage::RenewAccept(TenTenOneRenewAccept { renew_accept })
            }
            Message::Channel(ChannelMessage::RenewConfirm(renew_confirm)) => {
                TenTenOneMessage::RenewConfirm(TenTenOneRenewConfirm { renew_confirm })
            }
            Message::Channel(ChannelMessage::RenewFinalize(renew_finalize)) => {
                TenTenOneMessage::RenewFinalize(TenTenOneRenewFinalize { renew_finalize })
            }
            Message::Channel(ChannelMessage::RenewRevoke(renew_revoke)) => {
                TenTenOneMessage::RenewRevoke(TenTenOneRenewRevoke { renew_revoke })
            }
            Message::Channel(ChannelMessage::CollaborativeCloseOffer(
                collaborative_close_offer,
            )) => TenTenOneMessage::CollaborativeCloseOffer(TenTenOneCollaborativeCloseOffer {
                collaborative_close_offer,
            }),
            Message::Channel(ChannelMessage::Reject(reject)) => {
                TenTenOneMessage::Reject(TenTenOneReject { reject })
            }
            Message::OnChain(_)
            | Message::SubChannel(_)
            | Message::Channel(ChannelMessage::RenewOffer(_))
            | Message::Channel(ChannelMessage::SettleOffer(_))
            | Message::Channel(ChannelMessage::Offer(_)) => {
                unreachable!()
            }
        }
    }

    pub fn get_reference_id(&self) -> Option<ReferenceId> {
        match self {
            TenTenOneMessage::Offer(TenTenOneOfferChannel {
                offer_channel: OfferChannel { reference_id, .. },
                ..
            })
            | TenTenOneMessage::Accept(TenTenOneAcceptChannel {
                accept_channel: AcceptChannel { reference_id, .. },
            })
            | TenTenOneMessage::Sign(TenTenOneSignChannel {
                sign_channel: SignChannel { reference_id, .. },
            })
            | TenTenOneMessage::SettleOffer(TenTenOneSettleOffer {
                settle_offer: SettleOffer { reference_id, .. },
                ..
            })
            | TenTenOneMessage::SettleAccept(TenTenOneSettleAccept {
                settle_accept: SettleAccept { reference_id, .. },
            })
            | TenTenOneMessage::SettleConfirm(TenTenOneSettleConfirm {
                settle_confirm: SettleConfirm { reference_id, .. },
            })
            | TenTenOneMessage::SettleFinalize(TenTenOneSettleFinalize {
                settle_finalize: SettleFinalize { reference_id, .. },
            })
            | TenTenOneMessage::RenewOffer(TenTenOneRenewOffer {
                renew_offer: RenewOffer { reference_id, .. },
                ..
            })
            | TenTenOneMessage::RolloverOffer(TenTenOneRolloverOffer {
                renew_offer: RenewOffer { reference_id, .. },
            })
            | TenTenOneMessage::RenewAccept(TenTenOneRenewAccept {
                renew_accept: RenewAccept { reference_id, .. },
            })
            | TenTenOneMessage::RenewConfirm(TenTenOneRenewConfirm {
                renew_confirm: RenewConfirm { reference_id, .. },
            })
            | TenTenOneMessage::RenewFinalize(TenTenOneRenewFinalize {
                renew_finalize: RenewFinalize { reference_id, .. },
            })
            | TenTenOneMessage::RenewRevoke(TenTenOneRenewRevoke {
                renew_revoke: RenewRevoke { reference_id, .. },
            })
            | TenTenOneMessage::CollaborativeCloseOffer(TenTenOneCollaborativeCloseOffer {
                collaborative_close_offer: CollaborativeCloseOffer { reference_id, .. },
            })
            | TenTenOneMessage::Reject(TenTenOneReject {
                reject: Reject { reference_id, .. },
            }) => *reference_id,
        }
    }
}

impl From<TenTenOneMessage> for Message {
    fn from(value: TenTenOneMessage) -> Self {
        let msg = ChannelMessage::from(value);
        Message::Channel(msg)
    }
}

impl From<TenTenOneMessage> for ChannelMessage {
    fn from(value: TenTenOneMessage) -> Self {
        match value {
            TenTenOneMessage::Offer(TenTenOneOfferChannel { offer_channel, .. }) => {
                ChannelMessage::Offer(offer_channel)
            }
            TenTenOneMessage::Accept(TenTenOneAcceptChannel { accept_channel }) => {
                ChannelMessage::Accept(accept_channel)
            }
            TenTenOneMessage::Sign(TenTenOneSignChannel { sign_channel }) => {
                ChannelMessage::Sign(sign_channel)
            }
            TenTenOneMessage::SettleOffer(TenTenOneSettleOffer { settle_offer, .. }) => {
                ChannelMessage::SettleOffer(settle_offer)
            }
            TenTenOneMessage::SettleAccept(TenTenOneSettleAccept { settle_accept }) => {
                ChannelMessage::SettleAccept(settle_accept)
            }
            TenTenOneMessage::SettleConfirm(TenTenOneSettleConfirm { settle_confirm }) => {
                ChannelMessage::SettleConfirm(settle_confirm)
            }
            TenTenOneMessage::SettleFinalize(TenTenOneSettleFinalize { settle_finalize }) => {
                ChannelMessage::SettleFinalize(settle_finalize)
            }
            TenTenOneMessage::RenewOffer(TenTenOneRenewOffer { renew_offer, .. }) => {
                ChannelMessage::RenewOffer(renew_offer)
            }
            TenTenOneMessage::RolloverOffer(TenTenOneRolloverOffer { renew_offer }) => {
                ChannelMessage::RenewOffer(renew_offer)
            }
            TenTenOneMessage::RenewAccept(TenTenOneRenewAccept { renew_accept }) => {
                ChannelMessage::RenewAccept(renew_accept)
            }
            TenTenOneMessage::RenewConfirm(TenTenOneRenewConfirm { renew_confirm }) => {
                ChannelMessage::RenewConfirm(renew_confirm)
            }
            TenTenOneMessage::RenewFinalize(TenTenOneRenewFinalize { renew_finalize }) => {
                ChannelMessage::RenewFinalize(renew_finalize)
            }
            TenTenOneMessage::RenewRevoke(TenTenOneRenewRevoke { renew_revoke }) => {
                ChannelMessage::RenewRevoke(renew_revoke)
            }
            TenTenOneMessage::CollaborativeCloseOffer(TenTenOneCollaborativeCloseOffer {
                collaborative_close_offer,
            }) => ChannelMessage::CollaborativeCloseOffer(collaborative_close_offer),
            TenTenOneMessage::Reject(TenTenOneReject { reject }) => ChannelMessage::Reject(reject),
        }
    }
}

macro_rules! impl_type_writeable_for_enum {
    ($type_name: ident, {$($variant_name: ident),*}) => {
       impl Type for $type_name {
           fn type_id(&self) -> u16 {
               match self {
                   $($type_name::$variant_name(v) => v.type_id(),)*
               }
           }
       }

       impl Writeable for $type_name {
            fn write<W: Writer>(&self, writer: &mut W) -> Result<(), ::std::io::Error> {
                match self {
                   $($type_name::$variant_name(v) => v.write(writer),)*
                }
            }
       }
    };
}

macro_rules! impl_type {
    ($const_name: ident, $type_name: ident, $type_val: expr) => {
        /// The type prefix for an [`$type_name`] message.
        pub const $const_name: u16 = $type_val;

        impl Type for $type_name {
            fn type_id(&self) -> u16 {
                $const_name
            }
        }
    };
}

macro_rules! handle_read_tentenone_messages {
    ($msg_type:ident, $buffer:ident, $(($type_id:ident, $variant:ident)),*) => {{
        let decoded = match $msg_type {
            $(
                $type_id => TenTenOneMessage::$variant(Readable::read(&mut $buffer)?),
            )*
            _ => return Ok(None),
        };
        Ok(Some(WireMessage::Message(decoded)))
    }};
}

macro_rules! impl_serde_writeable {
    ($st:ident) => {
        impl Writeable for $st {
            fn write<W: Writer>(&self, w: &mut W) -> Result<(), ::std::io::Error> {
                let serialized = serde_json::to_string(self)?;
                write_string(&serialized, w)
            }
        }

        impl Readable for $st {
            fn read<R: std::io::Read>(r: &mut R) -> Result<Self, DecodeError> {
                let serialized = read_string(r)?;
                serde_json::from_str(&serialized).map_err(|_| DecodeError::InvalidValue)
            }
        }
    };
}

impl_type_writeable_for_enum!(WireMessage, { Message, SegmentStart, SegmentChunk });
impl_type_writeable_for_enum!(TenTenOneMessage,
{
    Reject,
    Offer,
    Accept,
    Sign,
    SettleOffer,
    SettleAccept,
    SettleConfirm,
    SettleFinalize,
    RenewOffer,
    RolloverOffer,
    RenewAccept,
    RenewConfirm,
    RenewFinalize,
    RenewRevoke,
    CollaborativeCloseOffer
});

impl_dlc_writeable!(TenTenOneReject, { (reject, writeable) });
impl_dlc_writeable!(TenTenOneOfferChannel, { (filled_with, writeable), (offer_channel, writeable) });
impl_dlc_writeable!(TenTenOneAcceptChannel, { (accept_channel, writeable) });
impl_dlc_writeable!(TenTenOneSignChannel, { (sign_channel, writeable) });
impl_dlc_writeable!(TenTenOneSettleOffer, { (order, writeable), (filled_with, writeable), (settle_offer, writeable) });
impl_dlc_writeable!(TenTenOneSettleAccept, { (settle_accept, writeable) });
impl_dlc_writeable!(TenTenOneSettleConfirm, { (settle_confirm, writeable) });
impl_dlc_writeable!(TenTenOneSettleFinalize, { (settle_finalize, writeable) });
impl_dlc_writeable!(TenTenOneRenewOffer, { (filled_with, writeable), (renew_offer, writeable) });
impl_dlc_writeable!(TenTenOneRolloverOffer, { (renew_offer, writeable) });
impl_dlc_writeable!(TenTenOneRenewAccept, { (renew_accept, writeable) });
impl_dlc_writeable!(TenTenOneRenewConfirm, { (renew_confirm, writeable) });
impl_dlc_writeable!(TenTenOneRenewFinalize, { (renew_finalize, writeable) });
impl_dlc_writeable!(TenTenOneRenewRevoke, { (renew_revoke, writeable) });
impl_dlc_writeable!(TenTenOneCollaborativeCloseOffer, {
    (collaborative_close_offer, writeable)
});

impl_type!(REJECT, TenTenOneReject, 43024);
impl_type!(OFFER_CHANNEL_TYPE, TenTenOneOfferChannel, 43000);
impl_type!(ACCEPT_CHANNEL_TYPE, TenTenOneAcceptChannel, 43002);
impl_type!(SIGN_CHANNEL_TYPE, TenTenOneSignChannel, 43004);
impl_type!(SETTLE_CHANNEL_OFFER_TYPE, TenTenOneSettleOffer, 43006);
impl_type!(SETTLE_CHANNEL_ACCEPT_TYPE, TenTenOneSettleAccept, 43008);
impl_type!(SETTLE_CHANNEL_CONFIRM_TYPE, TenTenOneSettleConfirm, 43010);
impl_type!(SETTLE_CHANNEL_FINALIZE_TYPE, TenTenOneSettleFinalize, 43012);
impl_type!(RENEW_CHANNEL_OFFER_TYPE, TenTenOneRenewOffer, 43014);
impl_type!(ROLLOVER_CHANNEL_OFFER_TYPE, TenTenOneRolloverOffer, 43028);
impl_type!(RENEW_CHANNEL_ACCEPT_TYPE, TenTenOneRenewAccept, 43016);
impl_type!(RENEW_CHANNEL_CONFIRM_TYPE, TenTenOneRenewConfirm, 43018);
impl_type!(RENEW_CHANNEL_FINALIZE_TYPE, TenTenOneRenewFinalize, 43020);
impl_type!(RENEW_CHANNEL_REVOKE_TYPE, TenTenOneRenewRevoke, 43026);
impl_type!(
    COLLABORATIVE_CLOSE_OFFER_TYPE,
    TenTenOneCollaborativeCloseOffer,
    43022
);

impl_serde_writeable!(Order);
impl_serde_writeable!(FilledWith);

fn read_tentenone_message<R: ::std::io::Read>(
    msg_type: u16,
    mut buffer: &mut R,
) -> Result<Option<WireMessage>, DecodeError> {
    handle_read_tentenone_messages!(
        msg_type,
        buffer,
        (REJECT, Reject),
        (OFFER_CHANNEL_TYPE, Offer),
        (ACCEPT_CHANNEL_TYPE, Accept),
        (SIGN_CHANNEL_TYPE, Sign),
        (SETTLE_CHANNEL_OFFER_TYPE, SettleOffer),
        (SETTLE_CHANNEL_ACCEPT_TYPE, SettleAccept),
        (SETTLE_CHANNEL_CONFIRM_TYPE, SettleConfirm),
        (SETTLE_CHANNEL_FINALIZE_TYPE, SettleFinalize),
        (RENEW_CHANNEL_OFFER_TYPE, RenewOffer),
        (ROLLOVER_CHANNEL_OFFER_TYPE, RolloverOffer),
        (RENEW_CHANNEL_ACCEPT_TYPE, RenewAccept),
        (RENEW_CHANNEL_CONFIRM_TYPE, RenewConfirm),
        (RENEW_CHANNEL_FINALIZE_TYPE, RenewFinalize),
        (RENEW_CHANNEL_REVOKE_TYPE, RenewRevoke),
        (COLLABORATIVE_CLOSE_OFFER_TYPE, CollaborativeCloseOffer)
    )
}

#[cfg(test)]
mod tests {
    use crate::commons;
    use crate::commons::ContractSymbol;
    use crate::commons::Direction;
    use crate::commons::OrderReason;
    use crate::commons::OrderState;
    use crate::commons::OrderType;
    use crate::message_handler::TenTenOneMessageHandler;
    use crate::message_handler::TenTenOneReject;
    use crate::message_handler::TenTenOneSettleOffer;
    use crate::node::event::NodeEventHandler;
    use anyhow::anyhow;
    use anyhow::Result;
    use dlc_manager::DlcChannelId;
    use dlc_messages::channel::Reject;
    use dlc_messages::channel::SettleOffer;
    use insta::assert_debug_snapshot;
    use lightning::ln::wire::CustomMessageReader;
    use lightning::ln::wire::Type;
    use lightning::util::ser::Readable;
    use lightning::util::ser::Writeable;
    use secp256k1::PublicKey;
    use std::io::Cursor;
    use std::str::FromStr;
    use std::sync::Arc;
    use time::OffsetDateTime;

    #[test]
    fn test_reject_writeable() {
        let reject = TenTenOneReject {
            reject: Reject {
                channel_id: DlcChannelId::default(),
                timestamp: 0,
                reference_id: None,
            },
        };

        let json_msg = handler_read_test(reject);
        assert_debug_snapshot!(json_msg);
    }

    #[test]
    fn test_settle_offer_impl_serde_writeable() {
        let settle_offer = TenTenOneSettleOffer {
            settle_offer: SettleOffer {
                channel_id: DlcChannelId::default(),
                counter_payout: 0,
                next_per_update_point: dummy_pubkey(),
                timestamp: 0,
                reference_id: None,
            },
            order: dummy_order(),
            filled_with: dummy_filled_with(),
        };

        let json_msg = handler_read_test(settle_offer);
        assert_debug_snapshot!(json_msg);
    }

    fn dummy_filled_with() -> commons::FilledWith {
        commons::FilledWith {
            order_id: Default::default(),
            expiry_timestamp: dummy_timestamp(),
            oracle_pk: dummy_x_only_pubkey(),
            matches: vec![],
        }
    }

    fn dummy_order() -> commons::Order {
        commons::Order {
            id: Default::default(),
            price: Default::default(),
            leverage: 0.0,
            contract_symbol: ContractSymbol::BtcUsd,
            trader_id: PublicKey::from_str(
                "02d5aa8fce495f6301b466594af056a46104dcdc6d735ec4793aa43108854cbd4a",
            )
            .unwrap(),
            direction: Direction::Long,
            quantity: Default::default(),
            order_type: OrderType::Market,
            timestamp: dummy_timestamp(),
            expiry: dummy_timestamp(),
            order_state: OrderState::Open,
            order_reason: OrderReason::Manual,
            stable: false,
        }
    }

    fn dummy_timestamp() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(0).unwrap()
    }

    fn dummy_pubkey() -> secp256k1_zkp::PublicKey {
        secp256k1_zkp::PublicKey::from_str(
            "02d5aa8fce495f6301b466594af056a46104dcdc6d735ec4793aa43108854cbd4a",
        )
        .unwrap()
    }

    fn dummy_x_only_pubkey() -> secp256k1::XOnlyPublicKey {
        secp256k1::XOnlyPublicKey::from_str(
            "cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115",
        )
        .unwrap()
    }

    fn handler_read_test<T: Writeable + Readable + Type + std::fmt::Debug>(
        msg: T,
    ) -> Result<String> {
        let mut buf = Vec::new();
        msg.type_id().write(&mut buf)?;
        msg.write(&mut buf)?;

        let handler = TenTenOneMessageHandler::new(Arc::new(NodeEventHandler::new()));
        let mut reader = Cursor::new(&mut buf);
        let message_type = <u16 as Readable>::read(&mut reader).map_err(|e| anyhow!("{e:#}"))?;
        let message = handler
            .read(message_type, &mut reader)
            .map_err(|e| anyhow!("{e:#}"))?
            .expect("to read message");

        let msg = serde_json::to_string(&message)?;
        Ok(msg)
    }
}
