use std::time::Duration;

use bdk::{blockchain::EsploraBlockchain, sled::Tree, TransactionDetails, Wallet};
use bitcoin::{Address, Amount, Network};
use time::OffsetDateTime;
use xtra::Actor as _;
use xtra_productivity::xtra_productivity;
use xtras::SendInterval;

const SYNC_INTERVAL: Duration = Duration::from_secs(3 * 60);

#[derive(Debug, Clone)]
pub struct WalletInfo {
    pub network: Network,
    pub balance: Amount,
    pub address: Address,
    pub last_updated_at: OffsetDateTime,
    pub transactions: Vec<TransactionDetails>,
}

pub struct WalletActor<B, DB> {
    wallet: Wallet<DB>,
    blockchain_client: B,
    cache: Option<WalletInfo>,
}

impl WalletActor<EsploraBlockchain, Tree> {
    pub fn new(wallet: Wallet<Tree>, blockchain_client: EsploraBlockchain) -> Self {
        Self {
            wallet,
            blockchain_client,
            cache: None,
        }
    }
}
