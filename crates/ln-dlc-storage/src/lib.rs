use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::SecretKey;
use dlc_manager::chain_monitor::ChainMonitor;
use dlc_manager::channel::accepted_channel::AcceptedChannel;
use dlc_manager::channel::offered_channel::OfferedChannel;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::signed_channel::SignedChannelStateType;
use dlc_manager::channel::Channel;
use dlc_manager::channel::ClosedChannel;
use dlc_manager::channel::ClosedPunishedChannel;
use dlc_manager::channel::ClosingChannel;
use dlc_manager::channel::FailedAccept;
use dlc_manager::channel::FailedSign;
use dlc_manager::channel::SettledClosingChannel;
use dlc_manager::contract::accepted_contract::AcceptedContract;
use dlc_manager::contract::offered_contract::OfferedContract;
use dlc_manager::contract::ser::Serializable;
use dlc_manager::contract::signed_contract::SignedContract;
use dlc_manager::contract::ClosedContract;
use dlc_manager::contract::Contract;
use dlc_manager::contract::FailedAcceptContract;
use dlc_manager::contract::FailedSignContract;
use dlc_manager::contract::PreClosedContract;
use dlc_manager::error::Error;
use dlc_manager::subchannel::SubChannel;
use dlc_manager::subchannel::SubChannelState;
use dlc_manager::ContractId;
use dlc_manager::DlcChannelId;
use dlc_manager::ReferenceId;
use lightning::ln::ChannelId;
use lightning::util::ser::Readable;
use lightning::util::ser::Writeable;
use std::convert::TryInto;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::string::ToString;
use std::sync::mpsc;

pub mod sled;

// Kinds.

const CONTRACT: u8 = 1;
const CHANNEL: u8 = 2;
const CHAIN_MONITOR: u8 = 3;
const KEY_PAIR: u8 = 6;
const SUB_CHANNEL: u8 = 7;
const ACTION: u8 = 9;

const CHAIN_MONITOR_KEY: &str = "chain_monitor";

pub trait WalletStorage {
    fn upsert_key_pair(&self, public_key: &PublicKey, privkey: &SecretKey) -> Result<()>;
    fn get_priv_key_for_pubkey(&self, public_key: &PublicKey) -> Result<Option<SecretKey>>;
}

pub struct KeyValue {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

pub trait DlcStoreProvider {
    /// Read the object from a kv store by the given key
    fn read(&self, kind: u8, key: Option<Vec<u8>>) -> Result<Vec<KeyValue>>;

    fn write(&self, kind: u8, key: Vec<u8>, value: Vec<u8>) -> Result<()>;

    fn delete(&self, kind: u8, key: Option<Vec<u8>>) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DlcChannelEvent {
    Offered(Option<ReferenceId>),
    Accepted(Option<ReferenceId>),
    Established(Option<ReferenceId>),
    SettledOffered(Option<ReferenceId>),
    SettledReceived(Option<ReferenceId>),
    SettledAccepted(Option<ReferenceId>),
    SettledConfirmed(Option<ReferenceId>),
    Settled(Option<ReferenceId>),
    SettledClosing(Option<ReferenceId>),
    RenewOffered(Option<ReferenceId>),
    RenewAccepted(Option<ReferenceId>),
    RenewConfirmed(Option<ReferenceId>),
    RenewFinalized(Option<ReferenceId>),
    Closing(Option<ReferenceId>),
    CollaborativeCloseOffered(Option<ReferenceId>),
    Closed(Option<ReferenceId>),
    CounterClosed(Option<ReferenceId>),
    ClosedPunished(Option<ReferenceId>),
    CollaborativelyClosed(Option<ReferenceId>),
    FailedAccept(Option<ReferenceId>),
    FailedSign(Option<ReferenceId>),
    Cancelled(Option<ReferenceId>),
    Deleted(Option<ReferenceId>),
}

impl DlcChannelEvent {
    pub fn get_reference_id(&self) -> Option<ReferenceId> {
        *match self {
            DlcChannelEvent::Offered(reference_id) => reference_id,
            DlcChannelEvent::Accepted(reference_id) => reference_id,
            DlcChannelEvent::Established(reference_id) => reference_id,
            DlcChannelEvent::SettledOffered(reference_id) => reference_id,
            DlcChannelEvent::SettledReceived(reference_id) => reference_id,
            DlcChannelEvent::SettledAccepted(reference_id) => reference_id,
            DlcChannelEvent::SettledConfirmed(reference_id) => reference_id,
            DlcChannelEvent::Settled(reference_id) => reference_id,
            DlcChannelEvent::SettledClosing(reference_id) => reference_id,
            DlcChannelEvent::RenewOffered(reference_id) => reference_id,
            DlcChannelEvent::RenewAccepted(reference_id) => reference_id,
            DlcChannelEvent::RenewConfirmed(reference_id) => reference_id,
            DlcChannelEvent::RenewFinalized(reference_id) => reference_id,
            DlcChannelEvent::Closing(reference_id) => reference_id,
            DlcChannelEvent::CollaborativeCloseOffered(reference_id) => reference_id,
            DlcChannelEvent::Closed(reference_id) => reference_id,
            DlcChannelEvent::CounterClosed(reference_id) => reference_id,
            DlcChannelEvent::ClosedPunished(reference_id) => reference_id,
            DlcChannelEvent::CollaborativelyClosed(reference_id) => reference_id,
            DlcChannelEvent::FailedAccept(reference_id) => reference_id,
            DlcChannelEvent::FailedSign(reference_id) => reference_id,
            DlcChannelEvent::Cancelled(reference_id) => reference_id,
            DlcChannelEvent::Deleted(reference_id) => reference_id,
        }
    }
}

/// Implementation of the dlc storage interface.
pub struct DlcStorageProvider<K> {
    store: K,
    event_sender: mpsc::Sender<DlcChannelEvent>,
}

macro_rules! convertible_enum {
    (enum $name:ident {
        $($vname:ident $(= $val:expr)? $(; $subprefix:ident, $subfield:ident)?,)*;
        $($tname:ident $(= $tval:expr)?,)*
    }, $input:ident) => {
        #[derive(Debug)]
        enum $name {
            $($vname $(= $val)?,)*
            $($tname $(= $tval)?,)*
        }

        impl From<$name> for u8 {
            fn from(prefix: $name) -> u8 {
                prefix as u8
            }
        }

        impl TryFrom<u8> for $name {
            type Error = Error;

            fn try_from(v: u8) -> Result<Self, Self::Error> {
                match v {
                    $(x if x == u8::from($name::$vname) => Ok($name::$vname),)*
                    $(x if x == u8::from($name::$tname) => Ok($name::$tname),)*
                    x => Err(Error::StorageError(format!("Unknown prefix {}", x))),
                }
            }
        }

        impl $name {
            fn get_prefix(input: &$input) -> u8 {
                let prefix = match input {
                    $($input::$vname(_) => $name::$vname,)*
                    $($input::$tname{..} => $name::$tname,)*
                };
                prefix.into()
            }
        }
    }
}

convertible_enum!(
    enum ContractPrefix {
        Offered = 1,
        Accepted,
        Signed,
        Confirmed,
        PreClosed,
        Closed,
        FailedAccept,
        FailedSign,
        Refunded,
        Rejected,;
    },
    Contract
);

convertible_enum!(
    enum ChannelPrefix {
        Offered = 1,
        Accepted,
        Signed; SignedChannelPrefix, state,
        Closing,
        Closed,
        CounterClosed,
        ClosedPunished,
        CollaborativelyClosed,
        FailedAccept,
        FailedSign,
        Cancelled,
        SettledClosing,;
    },
    Channel
);

convertible_enum!(
    enum SignedChannelPrefix {;
        Established = 1,
        SettledOffered,
        SettledReceived,
        SettledAccepted,
        SettledConfirmed,
        Settled,
        Closing,
        CollaborativeCloseOffered,
        RenewAccepted,
        RenewOffered,
        RenewConfirmed,
        RenewFinalized,
        SettledClosing,
    },
    SignedChannelStateType
);

convertible_enum!(
    enum SubChannelPrefix {;
        Offered = 1,
        Accepted,
        Confirmed,
        Finalized,
        Signed,
        Closing,
        OnChainClosed,
        CounterOnChainClosed,
        CloseOffered,
        CloseAccepted,
        CloseConfirmed,
        OffChainClosed,
        ClosedPunished,
        Rejected,
    },
    SubChannelState
);

fn to_storage_error<T>(e: T) -> Error
where
    T: std::fmt::Display,
{
    Error::StorageError(e.to_string())
}

impl<K: DlcStoreProvider> DlcStorageProvider<K> {
    /// Creates a new instance of a DlcStorageProvider
    pub fn new(store: K, event_sender: mpsc::Sender<DlcChannelEvent>) -> Self {
        DlcStorageProvider {
            store,
            event_sender,
        }
    }

    fn insert_contract(
        &self,
        serialized: Vec<u8>,
        contract: &Contract,
    ) -> Result<Option<Vec<u8>>, Error> {
        match contract {
            a @ Contract::Accepted(_) | a @ Contract::Signed(_) => {
                self.store
                    .delete(CONTRACT, Some(a.get_temporary_id().to_vec()))
                    .map_err(to_storage_error)?;
            }
            _ => {}
        };

        self.store
            .write(CONTRACT, contract.get_id().to_vec(), serialized.clone())
            .map_err(to_storage_error)?;

        Ok(Some(serialized))
    }

    fn get_data_with_prefix<T: Serializable>(
        &self,
        data: &[Vec<u8>],
        prefix: &[u8],
        consume: Option<u64>,
    ) -> Result<Vec<T>, Error> {
        data.iter()
            .filter_map(|value| {
                let mut cursor = Cursor::new(value);
                let mut pref = vec![0u8; prefix.len()];
                cursor.read_exact(&mut pref).expect("Error reading prefix");
                if pref == prefix {
                    if let Some(c) = consume {
                        cursor.set_position(cursor.position() + c);
                    }

                    match T::deserialize(&mut cursor) {
                        Ok(deserialized) => Some(Ok(deserialized)),
                        Err(e) => {
                            tracing::error!("Failed to deserialize data: {e}");
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_raw_contracts(&self) -> Result<Vec<Vec<u8>>, Error> {
        let contracts = self
            .store
            .read(CONTRACT, None)
            .map_err(to_storage_error)?
            .into_iter()
            .map(|x| x.value)
            .collect();

        Ok(contracts)
    }
}

impl<K: DlcStoreProvider> dlc_manager::Storage for DlcStorageProvider<K> {
    fn get_contract(&self, contract_id: &ContractId) -> Result<Option<Contract>, Error> {
        match self
            .store
            .read(CONTRACT, Some(contract_id.to_vec()))
            .map_err(to_storage_error)?
            .first()
        {
            Some(res) => Ok(Some(deserialize_contract(&res.value)?)),
            None => Ok(None),
        }
    }

    fn get_contracts(&self) -> Result<Vec<Contract>, Error> {
        let contracts = self.store.read(CONTRACT, None).map_err(to_storage_error)?;

        let contracts = contracts
            .iter()
            .filter_map(|x| match deserialize_contract(&x.value) {
                Ok(contract) => Some(contract),
                Err(e) => {
                    log::error!("Failed to deserialize contract: {e}");
                    None
                }
            })
            .collect();

        Ok(contracts)
    }

    fn create_contract(&self, contract: &OfferedContract) -> Result<(), Error> {
        let serialized = serialize_contract(&Contract::Offered(contract.clone()))?;
        self.store
            .write(CONTRACT, contract.id.to_vec(), serialized)
            .map_err(to_storage_error)
    }

    fn delete_contract(&self, contract_id: &ContractId) -> Result<(), Error> {
        self.store
            .delete(CONTRACT, Some(contract_id.to_vec()))
            .map_err(to_storage_error)
    }

    fn update_contract(&self, contract: &Contract) -> Result<(), Error> {
        let serialized = serialize_contract(contract)?;

        match contract {
            a @ Contract::Accepted(_) | a @ Contract::Signed(_) => {
                self.store
                    .delete(CONTRACT, Some(a.get_temporary_id().to_vec()))
                    .map_err(to_storage_error)?;
            }
            _ => {}
        };

        self.store
            .write(CONTRACT, contract.get_id().to_vec(), serialized)
            .map_err(to_storage_error)
    }

    fn get_contract_offers(&self) -> Result<Vec<OfferedContract>, Error> {
        let contracts = self.get_raw_contracts()?;

        self.get_data_with_prefix(&contracts, &[ContractPrefix::Offered.into()], None)
    }

    fn get_signed_contracts(&self) -> Result<Vec<SignedContract>, Error> {
        let contracts = self.get_raw_contracts()?;

        self.get_data_with_prefix(&contracts, &[ContractPrefix::Signed.into()], None)
    }

    fn get_confirmed_contracts(&self) -> Result<Vec<SignedContract>, Error> {
        let contracts = self.get_raw_contracts()?;

        self.get_data_with_prefix(&contracts, &[ContractPrefix::Confirmed.into()], None)
    }

    fn get_preclosed_contracts(&self) -> Result<Vec<PreClosedContract>, Error> {
        let contracts = self.get_raw_contracts()?;

        self.get_data_with_prefix(&contracts, &[ContractPrefix::PreClosed.into()], None)
    }

    fn upsert_channel(&self, channel: Channel, contract: Option<Contract>) -> Result<(), Error> {
        let serialized = serialize_channel(&channel)?;

        let serialized_contract = match contract.as_ref() {
            Some(c) => Some(serialize_contract(c)?),
            None => None,
        };

        match &channel {
            a @ Channel::Accepted(_) | a @ Channel::Signed(_) => {
                self.store
                    .delete(CHANNEL, Some(a.get_temporary_id().to_vec()))
                    .map_err(to_storage_error)?;
            }
            _ => {}
        };

        self.store
            .write(CHANNEL, channel.get_id().to_vec(), serialized)
            .map_err(to_storage_error)?;

        if let Some(contract) = contract.as_ref() {
            self.insert_contract(
                serialized_contract.expect("to have the serialized version"),
                contract,
            )?;
        }

        let dlc_channel_event = DlcChannelEvent::from(channel);
        if let Err(e) = self.event_sender.send(dlc_channel_event) {
            tracing::error!(
                ?dlc_channel_event,
                "Failed to send dlc channel event. Error: {e:#}"
            );
        }

        Ok(())
    }

    fn delete_channel(&self, channel_id: &DlcChannelId) -> Result<(), Error> {
        let channel = self.get_channel(channel_id)?;

        self.store
            .delete(CHANNEL, Some(channel_id.to_vec()))
            .map_err(to_storage_error)?;

        let dlc_channel_event =
            DlcChannelEvent::Deleted(channel.and_then(|channel| channel.get_reference_id()));

        if let Err(e) = self.event_sender.send(dlc_channel_event) {
            tracing::error!(
                ?dlc_channel_event,
                "Failed to send dlc channel event. Error: {e:#}"
            );
        }

        Ok(())
    }

    fn get_channel(&self, channel_id: &DlcChannelId) -> Result<Option<Channel>, Error> {
        match self
            .store
            .read(CHANNEL, Some(channel_id.to_vec()))
            .map_err(to_storage_error)?
            .first()
        {
            Some(res) => Ok(Some(deserialize_channel(&res.value)?)),
            None => Ok(None),
        }
    }

    fn get_signed_channels(
        &self,
        channel_state: Option<SignedChannelStateType>,
    ) -> Result<Vec<SignedChannel>, Error> {
        let (prefix, consume) = if let Some(state) = &channel_state {
            (
                vec![
                    ChannelPrefix::Signed.into(),
                    SignedChannelPrefix::get_prefix(state),
                ],
                None,
            )
        } else {
            (vec![ChannelPrefix::Signed.into()], Some(1))
        };

        let channels = self
            .store
            .read(CHANNEL, None)
            .map_err(to_storage_error)?
            .into_iter()
            .map(|x| x.value)
            .collect::<Vec<Vec<u8>>>();

        let channels = self.get_data_with_prefix(&channels, &prefix, consume)?;

        Ok(channels)
    }

    fn get_offered_channels(&self) -> Result<Vec<OfferedChannel>, Error> {
        let channels = self
            .store
            .read(CHANNEL, None)
            .map_err(to_storage_error)?
            .into_iter()
            .map(|x| x.value)
            .collect::<Vec<Vec<u8>>>();

        self.get_data_with_prefix(&channels, &[ChannelPrefix::Offered.into()], None)
    }

    fn get_settled_closing_channels(&self) -> Result<Vec<SettledClosingChannel>, Error> {
        let channels = self
            .store
            .read(CHANNEL, None)
            .map_err(to_storage_error)?
            .into_iter()
            .map(|x| x.value)
            .collect::<Vec<Vec<u8>>>();

        self.get_data_with_prefix(&channels, &[ChannelPrefix::SettledClosing.into()], None)
    }

    fn persist_chain_monitor(&self, monitor: &ChainMonitor) -> Result<(), Error> {
        self.store
            .write(
                CHAIN_MONITOR,
                CHAIN_MONITOR_KEY.to_string().into_bytes(),
                monitor.serialize()?,
            )
            .map_err(|e| Error::StorageError(format!("Error writing chain monitor: {e}")))
    }

    fn get_chain_monitor(&self) -> Result<Option<ChainMonitor>, Error> {
        let chain_monitors = self
            .store
            .read(
                CHAIN_MONITOR,
                Some(CHAIN_MONITOR_KEY.to_string().into_bytes()),
            )
            .map_err(|e| Error::StorageError(format!("Error reading chain monitor: {e}")))?;

        let serialized = chain_monitors.first();
        let deserialized = match serialized {
            Some(s) => Some(
                ChainMonitor::deserialize(&mut ::std::io::Cursor::new(s.value.clone()))
                    .map_err(to_storage_error)?,
            ),
            None => None,
        };
        Ok(deserialized)
    }

    fn upsert_sub_channel(&self, subchannel: &SubChannel) -> Result<(), Error> {
        let serialized = serialize_sub_channel(subchannel)?;

        self.store
            .write(SUB_CHANNEL, subchannel.channel_id.0.to_vec(), serialized)
            .map_err(to_storage_error)
    }

    fn get_sub_channel(&self, channel_id: ChannelId) -> Result<Option<SubChannel>, Error> {
        match self
            .store
            .read(SUB_CHANNEL, Some(channel_id.0.to_vec()))
            .map_err(to_storage_error)?
            .first()
        {
            Some(res) => Ok(Some(deserialize_sub_channel(&res.value)?)),
            None => Ok(None),
        }
    }

    fn get_sub_channels(&self) -> Result<Vec<SubChannel>, Error> {
        Ok(self
            .store
            .read(SUB_CHANNEL, None)
            .map_err(to_storage_error)?
            .iter()
            .filter_map(|x| match deserialize_sub_channel(&x.value) {
                Ok(sub_channel) => Some(sub_channel),
                Err(e) => {
                    tracing::error!("Failed to deserialize subchannel: {e}");
                    None
                }
            })
            .collect::<Vec<SubChannel>>())
    }

    fn get_offered_sub_channels(&self) -> Result<Vec<SubChannel>, Error> {
        let sub_channels = self
            .store
            .read(SUB_CHANNEL, None)
            .map_err(to_storage_error)?
            .into_iter()
            .map(|x| x.value)
            .collect::<Vec<Vec<u8>>>();

        self.get_data_with_prefix(&sub_channels, &[SubChannelPrefix::Offered.into()], None)
    }

    fn save_sub_channel_actions(
        &self,
        actions: &[dlc_manager::sub_channel_manager::Action],
    ) -> Result<(), Error> {
        let mut buf = Vec::new();

        for action in actions {
            action.write(&mut buf)?;
        }

        self.store
            .write(ACTION, "action".to_string().into_bytes(), buf)
            .map_err(to_storage_error)
    }

    fn get_sub_channel_actions(
        &self,
    ) -> Result<Vec<dlc_manager::sub_channel_manager::Action>, Error> {
        let actions = self.store.read(ACTION, None).map_err(to_storage_error)?;

        let buf = match actions.first() {
            Some(buf) if !buf.value.is_empty() => buf,
            Some(_) | None => return Ok(Vec::new()),
        };

        debug_assert!(!buf.value.is_empty());

        let len = buf.value.len();

        let mut res = Vec::new();
        let mut cursor = Cursor::new(buf.value.clone());

        while (cursor.position() as usize) < len - 1 {
            let action = Readable::read(&mut cursor).map_err(to_storage_error)?;
            res.push(action);
        }

        Ok(res)
    }

    fn get_channels(&self) -> Result<Vec<Channel>, Error> {
        Ok(self
            .store
            .read(CHANNEL, None)
            .map_err(to_storage_error)?
            .iter()
            .filter_map(|x| match deserialize_channel(&x.value) {
                Ok(channel) => Some(channel),
                Err(e) => {
                    tracing::error!("Failed to deserialize dlc channel: {e}");
                    None
                }
            })
            .collect::<Vec<Channel>>())
    }
}

impl<K: DlcStoreProvider> WalletStorage for DlcStorageProvider<K> {
    fn upsert_key_pair(&self, public_key: &PublicKey, privkey: &SecretKey) -> Result<()> {
        self.store.write(
            KEY_PAIR,
            public_key.serialize().to_vec(),
            privkey.secret_bytes().to_vec(),
        )
    }

    fn get_priv_key_for_pubkey(&self, public_key: &PublicKey) -> Result<Option<SecretKey>> {
        let priv_key = self
            .store
            .read(KEY_PAIR, None)?
            .iter()
            .filter_map(|x| {
                if x.key == public_key.serialize().to_vec() {
                    Some(SecretKey::from_slice(&x.value).expect("a valid secret key"))
                } else {
                    None
                }
            })
            .collect::<Vec<SecretKey>>()
            .first()
            .cloned();

        Ok(priv_key)
    }
}

fn serialize_contract(contract: &Contract) -> Result<Vec<u8>, Error> {
    let serialized = match contract {
        Contract::Offered(o) | Contract::Rejected(o) => o.serialize(),
        Contract::Accepted(o) => o.serialize(),
        Contract::Signed(o) | Contract::Confirmed(o) | Contract::Refunded(o) => o.serialize(),
        Contract::FailedAccept(c) => c.serialize(),
        Contract::FailedSign(c) => c.serialize(),
        Contract::PreClosed(c) => c.serialize(),
        Contract::Closed(c) => c.serialize(),
    };
    let mut serialized = serialized?;
    let mut res = Vec::with_capacity(serialized.len() + 1);
    res.push(ContractPrefix::get_prefix(contract));
    res.append(&mut serialized);
    Ok(res)
}

fn deserialize_contract(buff: &Vec<u8>) -> Result<Contract, Error> {
    let mut cursor = ::std::io::Cursor::new(buff);
    let mut prefix = [0u8; 1];
    cursor.read_exact(&mut prefix)?;
    let contract_prefix: ContractPrefix = prefix[0].try_into()?;
    let contract = match contract_prefix {
        ContractPrefix::Offered => {
            Contract::Offered(OfferedContract::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ContractPrefix::Accepted => Contract::Accepted(
            AcceptedContract::deserialize(&mut cursor).map_err(to_storage_error)?,
        ),
        ContractPrefix::Signed => {
            Contract::Signed(SignedContract::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ContractPrefix::Confirmed => {
            Contract::Confirmed(SignedContract::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ContractPrefix::PreClosed => Contract::PreClosed(
            PreClosedContract::deserialize(&mut cursor).map_err(to_storage_error)?,
        ),
        ContractPrefix::Closed => {
            Contract::Closed(ClosedContract::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ContractPrefix::FailedAccept => Contract::FailedAccept(
            FailedAcceptContract::deserialize(&mut cursor).map_err(to_storage_error)?,
        ),
        ContractPrefix::FailedSign => Contract::FailedSign(
            FailedSignContract::deserialize(&mut cursor).map_err(to_storage_error)?,
        ),
        ContractPrefix::Refunded => {
            Contract::Refunded(SignedContract::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ContractPrefix::Rejected => {
            Contract::Rejected(OfferedContract::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
    };
    Ok(contract)
}

fn serialize_channel(channel: &Channel) -> Result<Vec<u8>, ::std::io::Error> {
    let serialized = match channel {
        Channel::Offered(o) => o.serialize(),
        Channel::Accepted(a) => a.serialize(),
        Channel::Signed(s) => s.serialize(),
        Channel::FailedAccept(f) => f.serialize(),
        Channel::FailedSign(f) => f.serialize(),
        Channel::Closing(c) => c.serialize(),
        Channel::SettledClosing(c) => c.serialize(),
        Channel::Closed(c) | Channel::CounterClosed(c) | Channel::CollaborativelyClosed(c) => {
            c.serialize()
        }
        Channel::ClosedPunished(c) => c.serialize(),
        Channel::Cancelled(o) => o.serialize(),
    };
    let mut serialized = serialized?;
    let mut res = Vec::with_capacity(serialized.len() + 1);
    res.push(ChannelPrefix::get_prefix(channel));
    if let Channel::Signed(s) = channel {
        res.push(SignedChannelPrefix::get_prefix(&s.state.get_type()))
    }
    res.append(&mut serialized);
    Ok(res)
}

fn deserialize_channel(buff: &Vec<u8>) -> Result<Channel, Error> {
    let mut cursor = ::std::io::Cursor::new(buff);
    let mut prefix = [0u8; 1];
    cursor.read_exact(&mut prefix)?;
    let channel_prefix: ChannelPrefix = prefix[0].try_into()?;
    let channel = match channel_prefix {
        ChannelPrefix::Offered => {
            Channel::Offered(OfferedChannel::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ChannelPrefix::Accepted => {
            Channel::Accepted(AcceptedChannel::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ChannelPrefix::Signed => {
            // Skip the channel state prefix.
            cursor.set_position(cursor.position() + 1);
            Channel::Signed(SignedChannel::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ChannelPrefix::FailedAccept => {
            Channel::FailedAccept(FailedAccept::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ChannelPrefix::FailedSign => {
            Channel::FailedSign(FailedSign::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ChannelPrefix::Closing => {
            Channel::Closing(ClosingChannel::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ChannelPrefix::SettledClosing => Channel::SettledClosing(
            SettledClosingChannel::deserialize(&mut cursor).map_err(to_storage_error)?,
        ),
        ChannelPrefix::Closed => {
            Channel::Closed(ClosedChannel::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
        ChannelPrefix::CollaborativelyClosed => Channel::CollaborativelyClosed(
            ClosedChannel::deserialize(&mut cursor).map_err(to_storage_error)?,
        ),
        ChannelPrefix::CounterClosed => Channel::CounterClosed(
            ClosedChannel::deserialize(&mut cursor).map_err(to_storage_error)?,
        ),
        ChannelPrefix::ClosedPunished => Channel::ClosedPunished(
            ClosedPunishedChannel::deserialize(&mut cursor).map_err(to_storage_error)?,
        ),
        ChannelPrefix::Cancelled => {
            Channel::Cancelled(OfferedChannel::deserialize(&mut cursor).map_err(to_storage_error)?)
        }
    };
    Ok(channel)
}

fn serialize_sub_channel(sub_channel: &SubChannel) -> Result<Vec<u8>, ::std::io::Error> {
    let prefix = SubChannelPrefix::get_prefix(&sub_channel.state);
    let mut buf = Vec::new();

    buf.push(prefix);
    buf.append(&mut sub_channel.serialize()?);

    Ok(buf)
}

fn deserialize_sub_channel(buff: &Vec<u8>) -> Result<SubChannel, Error> {
    let mut cursor = ::std::io::Cursor::new(buff);
    // Skip prefix
    cursor.seek(SeekFrom::Start(1))?;
    SubChannel::deserialize(&mut cursor).map_err(to_storage_error)
}

impl From<Channel> for DlcChannelEvent {
    fn from(value: Channel) -> Self {
        match value {
            Channel::Offered(OfferedChannel { reference_id, .. }) => {
                DlcChannelEvent::Offered(reference_id)
            }
            Channel::Accepted(AcceptedChannel { reference_id, .. }) => {
                DlcChannelEvent::Accepted(reference_id)
            }
            Channel::Signed(SignedChannel {
                reference_id,
                state,
                ..
            }) => match state {
                SignedChannelState::Established { .. } => {
                    DlcChannelEvent::Established(reference_id)
                }
                SignedChannelState::SettledOffered { .. } => {
                    DlcChannelEvent::SettledOffered(reference_id)
                }
                SignedChannelState::SettledReceived { .. } => {
                    DlcChannelEvent::SettledReceived(reference_id)
                }
                SignedChannelState::SettledAccepted { .. } => {
                    DlcChannelEvent::SettledAccepted(reference_id)
                }
                SignedChannelState::SettledConfirmed { .. } => {
                    DlcChannelEvent::SettledConfirmed(reference_id)
                }
                SignedChannelState::Settled { .. } => DlcChannelEvent::Settled(reference_id),
                SignedChannelState::RenewOffered { .. } => {
                    DlcChannelEvent::RenewOffered(reference_id)
                }
                SignedChannelState::RenewAccepted { .. } => {
                    DlcChannelEvent::RenewAccepted(reference_id)
                }
                SignedChannelState::RenewConfirmed { .. } => {
                    DlcChannelEvent::RenewConfirmed(reference_id)
                }
                SignedChannelState::RenewFinalized { .. } => {
                    DlcChannelEvent::RenewFinalized(reference_id)
                }
                SignedChannelState::Closing { .. } => DlcChannelEvent::Closing(reference_id),
                SignedChannelState::SettledClosing { .. } => {
                    DlcChannelEvent::SettledClosing(reference_id)
                }
                SignedChannelState::CollaborativeCloseOffered { .. } => {
                    DlcChannelEvent::CollaborativeCloseOffered(reference_id)
                }
            },
            Channel::Closing(ClosingChannel { reference_id, .. }) => {
                DlcChannelEvent::Closing(reference_id)
            }
            Channel::SettledClosing(SettledClosingChannel { reference_id, .. }) => {
                DlcChannelEvent::SettledClosing(reference_id)
            }
            Channel::Closed(ClosedChannel { reference_id, .. }) => {
                DlcChannelEvent::Closed(reference_id)
            }
            Channel::CounterClosed(ClosedChannel { reference_id, .. }) => {
                DlcChannelEvent::CounterClosed(reference_id)
            }
            Channel::ClosedPunished(ClosedPunishedChannel { reference_id, .. }) => {
                DlcChannelEvent::ClosedPunished(reference_id)
            }
            Channel::CollaborativelyClosed(ClosedChannel { reference_id, .. }) => {
                DlcChannelEvent::CollaborativelyClosed(reference_id)
            }
            Channel::FailedAccept(FailedAccept { reference_id, .. }) => {
                DlcChannelEvent::FailedAccept(reference_id)
            }
            Channel::FailedSign(FailedSign { reference_id, .. }) => {
                DlcChannelEvent::FailedSign(reference_id)
            }
            Channel::Cancelled(OfferedChannel { reference_id, .. }) => {
                DlcChannelEvent::Cancelled(reference_id)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sled::InMemoryDlcStoreProvider;
    use dlc_manager::channel::signed_channel::SignedChannelState;
    use dlc_manager::Storage;

    fn deserialize_object<T>(serialized: &[u8]) -> T
    where
        T: Serializable,
    {
        let mut cursor = std::io::Cursor::new(&serialized);
        T::deserialize(&mut cursor).unwrap()
    }

    #[test]
    fn create_contract_can_be_retrieved() {
        let serialized = include_bytes!("../test_files/Offered");
        let contract = deserialize_object(serialized);

        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        storage
            .create_contract(&contract)
            .expect("Error creating contract");

        let retrieved = storage
            .get_contract(&contract.id)
            .expect("Error retrieving contract.");

        if let Some(Contract::Offered(retrieved_offer)) = retrieved {
            assert_eq!(serialized[..], retrieved_offer.serialize().unwrap()[..]);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn update_contract_is_updated() {
        let serialized = include_bytes!("../test_files/Offered");
        let offered_contract = deserialize_object(serialized);
        let serialized = include_bytes!("../test_files/Accepted");
        let accepted_contract = deserialize_object(serialized);
        let accepted_contract = Contract::Accepted(accepted_contract);

        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        storage
            .create_contract(&offered_contract)
            .expect("Error creating contract");

        storage
            .update_contract(&accepted_contract)
            .expect("Error updating contract.");
        let retrieved = storage
            .get_contract(&accepted_contract.get_id())
            .expect("Error retrieving contract.");

        if let Some(Contract::Accepted(_)) = retrieved {
        } else {
            unreachable!();
        }
    }

    #[test]
    fn delete_contract_is_deleted() {
        let serialized = include_bytes!("../test_files/Offered");
        let contract = deserialize_object(serialized);

        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        storage
            .create_contract(&contract)
            .expect("Error creating contract");

        storage
            .delete_contract(&contract.id)
            .expect("Error deleting contract");

        assert!(storage
            .get_contract(&contract.id)
            .expect("Error querying contract")
            .is_none());
    }

    fn insert_offered_signed_and_confirmed(
        storage: &mut DlcStorageProvider<InMemoryDlcStoreProvider>,
    ) {
        let serialized = include_bytes!("../test_files/Offered");
        let offered_contract = deserialize_object(serialized);
        storage
            .create_contract(&offered_contract)
            .expect("Error creating contract");

        let serialized = include_bytes!("../test_files/Signed");
        let signed_contract = Contract::Signed(deserialize_object(serialized));
        storage
            .update_contract(&signed_contract)
            .expect("Error creating contract");
        let serialized = include_bytes!("../test_files/Signed1");
        let signed_contract = Contract::Signed(deserialize_object(serialized));
        storage
            .update_contract(&signed_contract)
            .expect("Error creating contract");

        let serialized = include_bytes!("../test_files/Confirmed");
        let confirmed_contract = Contract::Confirmed(deserialize_object(serialized));
        storage
            .update_contract(&confirmed_contract)
            .expect("Error creating contract");
        let serialized = include_bytes!("../test_files/Confirmed1");
        let confirmed_contract = Contract::Confirmed(deserialize_object(serialized));
        storage
            .update_contract(&confirmed_contract)
            .expect("Error creating contract");

        let serialized = include_bytes!("../test_files/PreClosed");
        let preclosed_contract = Contract::PreClosed(deserialize_object(serialized));
        storage
            .update_contract(&preclosed_contract)
            .expect("Error creating contract");
    }

    fn insert_offered_and_signed_channels(
        storage: &mut DlcStorageProvider<InMemoryDlcStoreProvider>,
    ) {
        let serialized = include_bytes!("../test_files/Offered");
        let offered_contract = deserialize_object(serialized);
        let serialized = include_bytes!("../test_files/OfferedChannel");
        let offered_channel = deserialize_object(serialized);
        storage
            .upsert_channel(
                Channel::Offered(offered_channel),
                Some(Contract::Offered(offered_contract)),
            )
            .expect("Error creating contract");

        let serialized = include_bytes!("../test_files/SignedChannelEstablished");
        let signed_channel = Channel::Signed(deserialize_object(serialized));
        storage
            .upsert_channel(signed_channel, None)
            .expect("Error creating contract");

        let serialized = include_bytes!("../test_files/SignedChannelSettled");
        let signed_channel = Channel::Signed(deserialize_object(serialized));
        storage
            .upsert_channel(signed_channel, None)
            .expect("Error creating contract");
    }

    fn insert_sub_channels(storage: &mut DlcStorageProvider<InMemoryDlcStoreProvider>) {
        let serialized = include_bytes!("../test_files/OfferedSubChannel");
        let offered_sub_channel = deserialize_object(serialized);
        storage
            .upsert_sub_channel(&offered_sub_channel)
            .expect("Error inserting sub channel");
        let serialized = include_bytes!("../test_files/OfferedSubChannel1");
        let offered_sub_channel = deserialize_object(serialized);
        storage
            .upsert_sub_channel(&offered_sub_channel)
            .expect("Error inserting sub channel");

        let serialized = include_bytes!("../test_files/SignedSubChannel");
        let signed_sub_channel = deserialize_object(serialized);
        storage
            .upsert_sub_channel(&signed_sub_channel)
            .expect("Error inserting sub channel");

        let serialized = include_bytes!("../test_files/AcceptedSubChannel");
        let accepted_sub_channel = deserialize_object(serialized);
        storage
            .upsert_sub_channel(&accepted_sub_channel)
            .expect("Error inserting sub channel");
    }

    #[test]
    fn get_signed_contracts_only_signed() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_signed_and_confirmed(&mut storage);

        let signed_contracts = storage
            .get_signed_contracts()
            .expect("Error retrieving signed contracts");

        assert_eq!(2, signed_contracts.len());
    }

    #[test]
    fn get_confirmed_contracts_only_confirmed() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_signed_and_confirmed(&mut storage);

        let confirmed_contracts = storage
            .get_confirmed_contracts()
            .expect("Error retrieving signed contracts");

        assert_eq!(2, confirmed_contracts.len());
    }

    #[test]
    fn get_offered_contracts_only_offered() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_signed_and_confirmed(&mut storage);

        let offered_contracts = storage
            .get_contract_offers()
            .expect("Error retrieving signed contracts");

        assert_eq!(1, offered_contracts.len());
    }

    #[test]
    fn get_preclosed_contracts_only_preclosed() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_signed_and_confirmed(&mut storage);

        let preclosed_contracts = storage
            .get_preclosed_contracts()
            .expect("Error retrieving preclosed contracts");

        assert_eq!(1, preclosed_contracts.len());
    }

    #[test]
    fn get_contracts_all_returned() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_signed_and_confirmed(&mut storage);

        let contracts = storage.get_contracts().expect("Error retrieving contracts");

        assert_eq!(6, contracts.len());
    }

    #[test]
    fn get_offered_channels_only_offered() {
        let (sender, receiver) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_and_signed_channels(&mut storage);

        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Offered(None));
        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Established(None));
        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Settled(None));

        let offered_channels = storage
            .get_offered_channels()
            .expect("Error retrieving offered channels");
        assert_eq!(1, offered_channels.len());
    }

    #[test]
    fn get_signed_established_channel_only_established() {
        let (sender, receiver) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_and_signed_channels(&mut storage);

        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Offered(None));
        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Established(None));
        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Settled(None));

        let signed_channels = storage
            .get_signed_channels(Some(SignedChannelStateType::Established))
            .expect("Error retrieving offered channels");
        assert_eq!(1, signed_channels.len());
        if let SignedChannelState::Established { .. } = &signed_channels[0].state {
            let channel_id = signed_channels[0].channel_id;
            storage.get_channel(&channel_id).unwrap();
        } else {
            panic!(
                "Expected established state got {:?}",
                &signed_channels[0].state
            );
        }
    }

    #[test]
    fn get_channel_by_id_returns_correct_channel() {
        let (sender, receiver) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_and_signed_channels(&mut storage);

        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Offered(None));
        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Established(None));
        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Settled(None));

        let serialized = include_bytes!("../test_files/AcceptedChannel");
        let accepted_channel: AcceptedChannel = deserialize_object(serialized);
        let channel_id = accepted_channel.channel_id;
        storage
            .upsert_channel(Channel::Accepted(accepted_channel), None)
            .expect("Error creating contract");

        let accepted_dlc_event = receiver.recv().unwrap();
        assert_eq!(accepted_dlc_event, DlcChannelEvent::Accepted(None));

        storage
            .get_channel(&channel_id)
            .expect("error retrieving previously inserted channel.")
            .expect("to have found the previously inserted channel.");
    }

    #[test]
    fn delete_channel_is_not_returned() {
        let (sender, receiver) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_offered_and_signed_channels(&mut storage);

        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Offered(None));
        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Established(None));
        assert_eq!(receiver.recv().unwrap(), DlcChannelEvent::Settled(None));

        let serialized = include_bytes!("../test_files/AcceptedChannel");
        let accepted_channel: AcceptedChannel = deserialize_object(serialized);
        let channel_id = accepted_channel.channel_id;
        storage
            .upsert_channel(Channel::Accepted(accepted_channel), None)
            .expect("Error creating contract");

        let accepted_dlc_event = receiver.recv().unwrap();
        assert_eq!(accepted_dlc_event, DlcChannelEvent::Accepted(None));

        storage
            .get_channel(&channel_id)
            .expect("could not retrieve previously inserted channel.");

        storage
            .delete_channel(&channel_id)
            .expect("to be able to delete the channel");

        let deleted_dlc_event = receiver.recv().unwrap();
        assert_eq!(deleted_dlc_event, DlcChannelEvent::Deleted(None));

        assert!(storage
            .get_channel(&channel_id)
            .expect("error getting channel.")
            .is_none());
    }

    #[test]
    fn persist_chain_monitor_test() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        let chain_monitor = ChainMonitor::new(123);

        storage
            .persist_chain_monitor(&chain_monitor)
            .expect("to be able to persist the chain monitor.");

        let retrieved = storage
            .get_chain_monitor()
            .expect("to be able to retrieve the chain monitor.")
            .expect("to have a persisted chain monitor.");

        assert_eq!(chain_monitor, retrieved);

        let chain_monitor2 = ChainMonitor::new(456);

        storage
            .persist_chain_monitor(&chain_monitor2)
            .expect("to be able to persist the chain monitor.");

        let retrieved2 = storage
            .get_chain_monitor()
            .expect("to be able to retrieve the chain monitor.")
            .expect("to have a persisted chain monitor.");

        assert_eq!(chain_monitor2, retrieved2);
    }

    #[test]
    fn get_offered_sub_channels_only_offered() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_sub_channels(&mut storage);

        let offered_sub_channels = storage
            .get_offered_sub_channels()
            .expect("Error retrieving offered sub channels");
        assert_eq!(2, offered_sub_channels.len());
    }

    #[test]
    fn get_sub_channels_all_returned() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let mut storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        insert_sub_channels(&mut storage);

        let offered_sub_channels = storage
            .get_sub_channels()
            .expect("Error retrieving offered sub channels");
        assert_eq!(4, offered_sub_channels.len());
    }

    #[test]
    fn save_actions_roundtip_test() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        let actions: Vec<_> =
            serde_json::from_str(include_str!("../test_files/sub_channel_actions.json")).unwrap();
        storage
            .save_sub_channel_actions(&actions)
            .expect("Error saving sub channel actions");
        let recovered = storage
            .get_sub_channel_actions()
            .expect("Error getting sub channel actions");
        assert_eq!(actions, recovered);
    }

    #[test]
    fn get_actions_unset_test() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        let actions = storage
            .get_sub_channel_actions()
            .expect("Error getting sub channel actions");
        assert_eq!(actions.len(), 0);
    }

    #[test]
    fn get_empty_actions_test() {
        let (sender, _) = mpsc::channel::<DlcChannelEvent>();
        let storage = DlcStorageProvider::new(InMemoryDlcStoreProvider::new(), sender);
        storage.save_sub_channel_actions(&[]).unwrap();
        let actions = storage
            .get_sub_channel_actions()
            .expect("Error getting sub channel actions");
        assert_eq!(actions.len(), 0);
    }
}
