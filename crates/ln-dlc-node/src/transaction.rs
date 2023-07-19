use bitcoin::Txid;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    pub txid: Txid,
    pub fee: u64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Display for Transaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        format!(
            "txid: {}, fees: {}, created_at: {}, updated_at: {}",
            self.txid, self.fee, self.created_at, self.updated_at
        )
        .fmt(f)
    }
}
