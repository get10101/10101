use crate::api::Balances;
use crate::api::WalletInfo;
use std::sync::Arc;

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node>,
}

impl Node {
    pub fn get_wallet_info_from_node(&self) -> WalletInfo {
        WalletInfo {
            balances: Balances {
                lightning: self.inner.get_ldk_balance().available,
                on_chain: self
                    .inner
                    .get_on_chain_balance()
                    .expect("balance")
                    .confirmed,
            },
            history: vec![], // TODO: sync history
        }
    }
}
