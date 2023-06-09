use crate::node::Node;
use crate::node::NodeInfo;
use crate::node::PaymentMap;
use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::XOnlyPublicKey;
use reqwest::Response;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::Serialize;
use std::net::SocketAddr;
use std::time::Duration;
use time::ext::NumericalDuration;
use time::OffsetDateTime;
use uuid::Uuid;

/// A 10101 test coordinator.
pub struct Coordinator {
    http_endpoint: SocketAddr,
    pubkey: PublicKey,
    p2p_address: SocketAddr,
}

#[derive(Serialize)]
pub enum Direction {
    Long,
    Short,
}

impl Coordinator {
    pub fn new(http_endpoint: &str, pubkey: &str, p2p_address: &str) -> Result<Self> {
        Ok(Coordinator {
            http_endpoint: http_endpoint.parse()?,
            pubkey: pubkey.parse()?,
            p2p_address: p2p_address.parse()?,
        })
    }

    pub fn new_public_regtest() -> Self {
        const HTTP_ENDPOINT: &str = "35.189.57.114:80";
        const PK: &str = "03507b924dae6595cfb78492489978127c5f1e3877848564de2015cd6d41375802";
        const P2P_ADDRESS: &str = "35.189.57.114:9045";

        Self::new(HTTP_ENDPOINT, PK, P2P_ADDRESS).expect("correct coordinator parameters")
    }

    pub fn info(&self) -> NodeInfo {
        NodeInfo {
            pubkey: self.pubkey,
            address: self.p2p_address,
        }
    }

    /// Instruct the 10101 coordinator node to open a private channel with the app node.
    ///
    /// Assumes that the app node is already connected to the coordinator.
    pub async fn open_channel(
        &self,
        app: &Node<PaymentMap>,
        coordinator_balance: u64,
        app_balance: u64,
    ) -> Result<()> {
        #[derive(Serialize)]
        struct OpenChannelRequest {
            target: TargetInfo,
            local_balance: u64,
            remote_balance: u64,
            is_public: bool,
        }

        #[derive(Serialize)]
        struct TargetInfo {
            pubkey: PublicKey,
        }

        tracing::info!(
            coordinator = ?self.info(),
            app = ?app.info,
            %coordinator_balance,
            %app_balance,
            "Opening channel between coordinator and target node",
        );

        self.post(
            "api/channels",
            &OpenChannelRequest {
                target: TargetInfo {
                    pubkey: app.info.pubkey,
                },
                local_balance: coordinator_balance,
                remote_balance: app_balance,
                is_public: false,
            },
        )
        .await?;

        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                if app.list_channels().iter().any(|channel| {
                    channel.is_channel_ready && channel.counterparty.node_id == self.info().pubkey
                }) {
                    break;
                }

                tracing::debug!("Waiting for channel to be usable");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            anyhow::Ok(())
        })
        .await??;

        tracing::info!(
            coordinator = ?self.info(),
            app = ?app.info,
            "Channel open between coordinator and app",
        );

        Ok(())
    }

    pub async fn post_trade(&self, app: &Node<PaymentMap>, direction: Direction) -> Result<()> {
        #[derive(Serialize)]
        pub struct TradeParams {
            pub pubkey: PublicKey,
            pub contract_symbol: String,
            pub leverage: f32,
            pub quantity: f32,
            pub direction: Direction,
            pub filled_with: FilledWith,
        }

        #[derive(Serialize)]
        pub struct FilledWith {
            pub order_id: Uuid,
            pub expiry_timestamp: OffsetDateTime,
            pub oracle_pk: XOnlyPublicKey,
            pub matches: Vec<Match>,
        }

        #[derive(Serialize)]
        pub struct Match {
            pub order_id: Uuid,
            pub quantity: Decimal,
            pub pubkey: PublicKey,
            pub execution_price: Decimal,
        }

        let order_id = Uuid::new_v4();
        let expiry_timestamp = OffsetDateTime::now_utc() + 1.days();
        let quantity = 20.0;

        let trade_params = TradeParams {
            pubkey: app.info.pubkey,
            contract_symbol: "BtcUsd".to_string(),
            leverage: 2.0,
            quantity,
            direction,
            filled_with: FilledWith {
                order_id,
                expiry_timestamp,
                oracle_pk: app.oracle_pk(),
                matches: vec![Match {
                    order_id: Uuid::new_v4(),
                    quantity: Decimal::from_f32(quantity).unwrap(),
                    pubkey: self.info().pubkey, // should be the maker's one, but there is no maker
                    execution_price: Decimal::from(30_000),
                }],
            },
        };

        self.post("api/trade", &trade_params).await?;

        tracing::info!("Sent trade request to coordinator successfully");

        Ok(())
    }

    async fn post<B>(&self, path: &str, json: &B) -> Result<Response>
    where
        B: Serialize,
    {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{}/{path}", self.http_endpoint))
            .json(json)
            .send()
            .await?;

        if !response.status().is_success() {
            let response_text = match response.text().await {
                Ok(text) => text,
                Err(err) => {
                    format!("could not decode response {err:#}")
                }
            };

            bail!(response_text)
        }

        Ok(response)
    }
}
