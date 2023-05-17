mod channel_close;
mod create;
mod multiple_payments;
mod offline_receiver;

#[derive(PartialEq)]
pub enum TestPath {
    // funding through an always on lightning node
    FundingAlwaysOnline,
    // funding through a mobile lightning node (on the same phone)
    FundingThroughMobile,
    // funding should fail (the HTLC fails)
    ExpectFundingFailure,
}
