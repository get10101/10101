use bitcoin::consensus::encode::serialize_hex;
use bitcoin::Txid;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    txid: Txid,
    fee: u64,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    raw: String,
}

impl Transaction {
    pub fn new(
        txid: Txid,
        fee: u64,
        created_at: OffsetDateTime,
        updated_at: OffsetDateTime,
        raw: String,
    ) -> Self {
        Self {
            txid,
            fee,
            created_at,
            updated_at,
            raw,
        }
    }

    pub fn txid(&self) -> Txid {
        self.txid
    }

    pub fn fee(&self) -> u64 {
        self.fee
    }

    pub fn with_fee(self, fee: u64) -> Self {
        Self {
            fee,
            updated_at: OffsetDateTime::now_utc(),
            ..self
        }
    }

    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    pub fn raw(&self) -> String {
        self.raw.clone()
    }
}

impl From<&bitcoin::Transaction> for Transaction {
    fn from(value: &bitcoin::Transaction) -> Self {
        let now = OffsetDateTime::now_utc();

        Self::new(value.txid(), 0, now, now, serialize_hex(value))
    }
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
