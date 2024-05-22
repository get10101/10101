use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use async_stream::stream;
use futures::Stream;
use futures::StreamExt;
use reqwest::Method;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[derive(Clone)]
pub struct LndBridge {
    pub client: reqwest::Client,
    pub endpoint: String,
    pub macaroon: String,
    pub secure: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InvoiceResult {
    pub result: Invoice,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Invoice {
    pub memo: String,
    #[serde(deserialize_with = "string_as_u64", serialize_with = "u64_as_string")]
    pub expiry: u64,
    #[serde(deserialize_with = "string_as_u64", serialize_with = "u64_as_string")]
    pub amt_paid_sat: u64,
    pub state: InvoiceState,
    pub payment_request: String,
    pub r_hash: String,
    #[serde(deserialize_with = "string_as_u64", serialize_with = "u64_as_string")]
    pub add_index: u64,
    #[serde(deserialize_with = "string_as_u64", serialize_with = "u64_as_string")]
    pub settle_index: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InvoiceParams {
    pub value: u64,
    pub memo: String,
    pub expiry: u64,
    pub hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InvoiceResponse {
    #[serde(deserialize_with = "string_as_u64", serialize_with = "u64_as_string")]
    pub add_index: u64,
    pub payment_addr: String,
    pub payment_request: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SettleInvoice {
    pub preimage: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CancelInvoice {
    pub payment_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum InvoiceState {
    #[serde(rename = "OPEN")]
    Open,
    #[serde(rename = "SETTLED")]
    Settled,
    #[serde(rename = "CANCELED")]
    Canceled,
    #[serde(rename = "ACCEPTED")]
    Accepted,
}

fn string_as_u64<'de, T, D>(de: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    String::deserialize(de)?
        .parse()
        .map_err(serde::de::Error::custom)
}

pub fn u64_as_string<S>(x: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&x.to_string())
}

impl LndBridge {
    pub fn new(endpoint: String, macaroon: String, secure: bool) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint,
            macaroon,
            secure,
        }
    }

    pub async fn settle_invoice(&self, preimage: String) -> Result<()> {
        let builder = self.client.request(
            Method::POST,
            format!(
                "{}://{}/v2/invoices/settle",
                if self.secure { "https" } else { "http" },
                self.endpoint
            ),
        );

        let resp = builder
            .header("content-type", "application/json")
            .header("Grpc-Metadata-macaroon", self.macaroon.clone())
            .json(&SettleInvoice { preimage })
            .send()
            .await?;

        resp.error_for_status()?.text().await?;

        Ok(())
    }

    pub async fn create_invoice(&self, params: InvoiceParams) -> Result<InvoiceResponse> {
        let builder = self.client.request(
            Method::POST,
            format!(
                "{}://{}/v2/invoices/hodl",
                if self.secure { "https" } else { "http" },
                self.endpoint
            ),
        );

        let resp = builder
            .header("content-type", "application/json")
            .header("Grpc-Metadata-macaroon", self.macaroon.clone())
            .json(&params)
            .send()
            .await?;

        let invoice: InvoiceResponse = resp.error_for_status()?.json().await?;

        Ok(invoice)
    }

    pub async fn cancel_invoice(&self, payment_hash: String) -> Result<()> {
        let builder = self.client.request(
            Method::POST,
            format!(
                "{}://{}/v2/invoices/cancel",
                if self.secure { "https" } else { "http" },
                self.endpoint
            ),
        );

        let resp = builder
            .header("content-type", "application/json")
            .header("Grpc-Metadata-macaroon", self.macaroon.clone())
            .json(&CancelInvoice { payment_hash })
            .send()
            .await?;

        resp.error_for_status()?.text().await?;

        Ok(())
    }

    /// Subscribes to an invoice update for a given `r_hash` to the lnd api.
    pub fn subscribe_to_invoice(
        &self,
        r_hash: String,
    ) -> impl Stream<Item = Result<Invoice>> + Unpin + '_ {
        let stream = stream! {
            tracing::debug!("Connecting to lnd websocket API");

            let url_str = &*format!("{}://{}/v2/invoices/subscribe/{r_hash}", if self.secure { "wss" } else { "ws" }, self.endpoint);
            let url = url::Url::parse(url_str)?;

            let mut req = url.into_client_request()?;
            let headers = req.headers_mut();
            headers.insert("Grpc-Metadata-macaroon", self.macaroon.parse().map_err(|e| anyhow!(format!("{e:#}")))?);

            let (mut connection, _) = tokio_tungstenite::connect_async(req)
                .await
                .context("Could not connect to websocket")?;

            tracing::info!("Connected to lnd websocket API");

            loop {
                match connection.next().await {
                    Some(Ok(msg)) => match msg {
                        tungstenite::Message::Text(text) => {
                            match serde_json::from_str::<InvoiceResult>(&text) {
                                Ok(invoice) => yield Ok(invoice.result),
                                Err(e) => yield Err(anyhow!(format!("{text}. Error: {e:#}")))
                            }
                        }
                        tungstenite::Message::Ping(_) => {
                            tracing::trace!("Received ping from lnd");
                        }
                        other => {
                            tracing::trace!("Unsupported message: {:?}", other);
                            continue;
                        }
                    },
                    None => return,
                    Some(Err(e)) => {
                        yield Err(anyhow!(e));
                        return;
                    }
                }
            }
        };

        stream.boxed()
    }
}
