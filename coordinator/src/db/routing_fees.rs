use crate::routing_fee;
use crate::schema::routing_fees;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use diesel;
use diesel::prelude::*;
use lightning::ln::ChannelId;
use time::OffsetDateTime;

#[derive(Insertable, Debug, PartialEq)]
#[diesel(table_name = routing_fees)]
struct NewRoutingFee {
    amount_msats: i64,
    prev_channel_id: Option<String>,
    next_channel_id: Option<String>,
}

#[derive(Queryable, Debug, Clone)]
#[diesel(table_name = routing_fees)]
struct RoutingFee {
    id: i32,
    amount_msats: i64,
    prev_channel_id: Option<String>,
    next_channel_id: Option<String>,
    created_at: OffsetDateTime,
}

pub fn insert(
    routing_fee: routing_fee::models::NewRoutingFee,
    conn: &mut PgConnection,
) -> Result<routing_fee::models::RoutingFee> {
    let routing_fee: NewRoutingFee = routing_fee.into();
    let routing_fee: RoutingFee = diesel::insert_into(routing_fees::table)
        .values(&routing_fee)
        .get_result(conn)?;

    Ok(routing_fee.into())
}

impl From<routing_fee::models::NewRoutingFee> for NewRoutingFee {
    fn from(value: routing_fee::models::NewRoutingFee) -> Self {
        Self {
            amount_msats: value.amount_msats as i64,
            prev_channel_id: value
                .prev_channel_id
                .map(|prev_channel_id| prev_channel_id.to_hex()),
            next_channel_id: value
                .next_channel_id
                .map(|next_channel_id| next_channel_id.to_hex()),
        }
    }
}

impl From<RoutingFee> for routing_fee::models::RoutingFee {
    fn from(value: RoutingFee) -> Self {
        Self {
            id: value.id,
            amount_msats: value.amount_msats as u64,
            prev_channel_id: value.prev_channel_id.map(|prev_channel_id| {
                let channel_id = hex::decode(prev_channel_id).expect("prev channel id to decode");
                let channel_id: [u8; 32] = channel_id.try_into().expect("to fit into 32 bytes");
                ChannelId(channel_id)
            }),
            next_channel_id: value.next_channel_id.map(|next_channel_id| {
                let channel_id = hex::decode(next_channel_id).expect("next channel id to decode");
                let channel_id: [u8; 32] = channel_id.try_into().expect("to fit into 32 bytes");
                ChannelId(channel_id)
            }),
            created_at: value.created_at,
        }
    }
}
