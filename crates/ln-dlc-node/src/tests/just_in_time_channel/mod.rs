mod channel_close;
mod create;
mod multiple_payments;
mod offline_receiver;

#[derive(PartialEq)]
pub enum TestPathFunding {
    // funding through an always on lightning node
    Online,
    // funding through a mobile lightning node (on the same phone)
    Mobile,
}

#[derive(PartialEq)]
pub enum TestPathChannelClose {
    Force { with_dlc: bool },
    Collaborative { with_dlc: bool },
}
