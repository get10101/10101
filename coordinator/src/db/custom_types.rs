use crate::db::bonus_status::BonusType;
use crate::db::dlc_channels::DlcChannelState;
use crate::db::dlc_messages::MessageType;
use crate::db::dlc_protocols::DlcProtocolState;
use crate::db::dlc_protocols::DlcProtocolType;
use crate::db::polls::PollType;
use crate::db::positions::ContractSymbol;
use crate::db::positions::PositionState;
use crate::schema::sql_types::BonusStatusType;
use crate::schema::sql_types::ContractSymbolType;
use crate::schema::sql_types::DirectionType;
use crate::schema::sql_types::DlcChannelStateType;
use crate::schema::sql_types::MessageTypeType;
use crate::schema::sql_types::PollTypeType;
use crate::schema::sql_types::PositionStateType;
use crate::schema::sql_types::ProtocolStateType;
use crate::schema::sql_types::ProtocolTypeType;
use diesel::deserialize;
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::serialize;
use diesel::serialize::IsNull;
use diesel::serialize::Output;
use diesel::serialize::ToSql;
use std::io::Write;
use trade::Direction;

impl ToSql<ContractSymbolType, Pg> for ContractSymbol {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            ContractSymbol::BtcUsd => out.write_all(b"BtcUsd")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<ContractSymbolType, Pg> for ContractSymbol {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"BtcUsd" => Ok(ContractSymbol::BtcUsd),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<PositionStateType, Pg> for PositionState {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            PositionState::Open => out.write_all(b"Open")?,
            PositionState::Closing => out.write_all(b"Closing")?,
            PositionState::Closed => out.write_all(b"Closed")?,
            PositionState::Rollover => out.write_all(b"Rollover")?,
            PositionState::Resizing => out.write_all(b"Resizing")?,
            PositionState::Proposed => out.write_all(b"Proposed")?,
            PositionState::Failed => out.write_all(b"Failed")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PositionStateType, Pg> for PositionState {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Open" => Ok(PositionState::Open),
            b"Closing" => Ok(PositionState::Closing),
            b"Closed" => Ok(PositionState::Closed),
            b"Rollover" => Ok(PositionState::Rollover),
            b"Resizing" => Ok(PositionState::Resizing),
            b"Proposed" => Ok(PositionState::Proposed),
            b"Failed" => Ok(PositionState::Failed),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<DirectionType, Pg> for Direction {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            Direction::Long => out.write_all(b"Long")?,
            Direction::Short => out.write_all(b"Short")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<DirectionType, Pg> for Direction {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Long" => Ok(Direction::Long),
            b"Short" => Ok(Direction::Short),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<MessageTypeType, Pg> for MessageType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            MessageType::Offer => out.write_all(b"Offer")?,
            MessageType::Accept => out.write_all(b"Accept")?,
            MessageType::Sign => out.write_all(b"Sign")?,
            MessageType::SettleOffer => out.write_all(b"SettleOffer")?,
            MessageType::SettleAccept => out.write_all(b"SettleAccept")?,
            MessageType::SettleConfirm => out.write_all(b"SettleConfirm")?,
            MessageType::SettleFinalize => out.write_all(b"SettleFinalize")?,
            MessageType::RenewOffer => out.write_all(b"RenewOffer")?,
            MessageType::RenewAccept => out.write_all(b"RenewAccept")?,
            MessageType::RenewConfirm => out.write_all(b"RenewConfirm")?,
            MessageType::RenewFinalize => out.write_all(b"RenewFinalize")?,
            MessageType::RenewRevoke => out.write_all(b"RenewRevoke")?,
            MessageType::CollaborativeCloseOffer => out.write_all(b"CollaborativeCloseOffer")?,
            MessageType::Reject => out.write_all(b"Reject")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<MessageTypeType, Pg> for MessageType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Offer" => Ok(MessageType::Offer),
            b"Accept" => Ok(MessageType::Accept),
            b"Sign" => Ok(MessageType::Sign),
            b"SettleOffer" => Ok(MessageType::SettleOffer),
            b"SettleAccept" => Ok(MessageType::SettleAccept),
            b"SettleConfirm" => Ok(MessageType::SettleConfirm),
            b"SettleFinalize" => Ok(MessageType::SettleFinalize),
            b"RenewOffer" => Ok(MessageType::RenewOffer),
            b"RenewAccept" => Ok(MessageType::RenewAccept),
            b"RenewConfirm" => Ok(MessageType::RenewConfirm),
            b"RenewFinalize" => Ok(MessageType::RenewFinalize),
            b"RenewRevoke" => Ok(MessageType::RenewRevoke),
            b"CollaborativeCloseOffer" => Ok(MessageType::CollaborativeCloseOffer),
            b"Reject" => Ok(MessageType::Reject),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<PollTypeType, Pg> for PollType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            PollType::SingleChoice => out.write_all(b"SingleChoice")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<PollTypeType, Pg> for PollType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"SingleChoice" => Ok(PollType::SingleChoice),
            _ => Err("Unrecognized enum variant for PollType".into()),
        }
    }
}

impl ToSql<ProtocolStateType, Pg> for DlcProtocolState {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            DlcProtocolState::Pending => out.write_all(b"Pending")?,
            DlcProtocolState::Success => out.write_all(b"Success")?,
            DlcProtocolState::Failed => out.write_all(b"Failed")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<ProtocolStateType, Pg> for DlcProtocolState {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Pending" => Ok(DlcProtocolState::Pending),
            b"Success" => Ok(DlcProtocolState::Success),
            b"Failed" => Ok(DlcProtocolState::Failed),
            _ => Err("Unrecognized enum variant for ProtocolStateType".into()),
        }
    }
}

impl ToSql<ProtocolTypeType, Pg> for DlcProtocolType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            DlcProtocolType::OpenChannel => out.write_all(b"open-channel")?,
            DlcProtocolType::Settle => out.write_all(b"settle")?,
            DlcProtocolType::OpenPosition => out.write_all(b"open-position")?,
            DlcProtocolType::Rollover => out.write_all(b"rollover")?,
            DlcProtocolType::Close => out.write_all(b"close")?,
            DlcProtocolType::ForceClose => out.write_all(b"force-close")?,
            DlcProtocolType::ResizePosition => out.write_all(b"resize-position")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<ProtocolTypeType, Pg> for DlcProtocolType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"open-channel" => Ok(DlcProtocolType::OpenChannel),
            b"settle" => Ok(DlcProtocolType::Settle),
            b"open-position" => Ok(DlcProtocolType::OpenPosition),
            b"rollover" => Ok(DlcProtocolType::Rollover),
            b"close" => Ok(DlcProtocolType::Close),
            b"force-close" => Ok(DlcProtocolType::ForceClose),
            b"resize-position" => Ok(DlcProtocolType::ResizePosition),
            _ => Err("Unrecognized enum variant for ProtocolTypeType".into()),
        }
    }
}

impl ToSql<DlcChannelStateType, Pg> for DlcChannelState {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match *self {
            DlcChannelState::Pending => out.write_all(b"Pending")?,
            DlcChannelState::Open => out.write_all(b"Open")?,
            DlcChannelState::Closing => out.write_all(b"Closing")?,
            DlcChannelState::Closed => out.write_all(b"Closed")?,
            DlcChannelState::Failed => out.write_all(b"Failed")?,
            DlcChannelState::Cancelled => out.write_all(b"Cancelled")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<DlcChannelStateType, Pg> for DlcChannelState {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Pending" => Ok(DlcChannelState::Pending),
            b"Open" => Ok(DlcChannelState::Open),
            b"Closing" => Ok(DlcChannelState::Closing),
            b"Closed" => Ok(DlcChannelState::Closed),
            b"Failed" => Ok(DlcChannelState::Failed),
            b"Cancelled" => Ok(DlcChannelState::Cancelled),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl ToSql<BonusStatusType, Pg> for BonusType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            BonusType::Referral => out.write_all(b"Referral")?,
            BonusType::Referent => out.write_all(b"Referent")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<BonusStatusType, Pg> for BonusType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"Referral" => Ok(BonusType::Referral),
            b"Referent" => Ok(BonusType::Referent),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}
