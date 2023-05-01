mod create;
mod force_close;
mod multiple_payments;
mod offline_receiver;

#[derive(PartialEq)]
pub enum TestPath {
    // funding through an always on lightning node
    OnlineFunding,
    // funding through a mobile lightning node (on the same phone)
    MobileFunding,
}
