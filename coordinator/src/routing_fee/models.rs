use dlc_manager::ChannelId;
use time::OffsetDateTime;

#[derive(Debug)]
pub struct NewRoutingFee {
    pub amount_msats: u64,
    pub prev_channel_id: Option<ChannelId>,
    pub next_channel_id: Option<ChannelId>,
}

#[derive(Debug)]
pub struct RoutingFee {
    pub id: i32,
    pub amount_msats: u64,
    pub prev_channel_id: Option<ChannelId>,
    pub next_channel_id: Option<ChannelId>,
    pub created_at: OffsetDateTime,
}
