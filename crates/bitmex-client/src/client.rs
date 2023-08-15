use crate::models::ContractSymbol;
use crate::models::GetPositionRequest;
use crate::models::Network;
use crate::models::OrdType;
use crate::models::Order;
use crate::models::Position;
use crate::models::PostOrderRequest;
use crate::models::Request;
use crate::models::Side;
use anyhow::bail;
use anyhow::Result;
use hex::encode as hexify;
use reqwest::Method;
use reqwest::Response;
use reqwest::Url;
use reqwest::{self};
use ring::hmac;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use serde_json::from_str;
use serde_json::to_string as to_jstring;
use serde_urlencoded::to_string as to_ustring;
use std::ops::Add;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Clone)]
pub struct Client {
    url: String,
    credentials: Option<Credentials>,
    client: reqwest::Client,
}

impl Client {
    pub fn new(network: Network) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: network.to_url(),
            credentials: None,
        }
    }

    pub fn with_credentials(self, api_key: impl ToString, secret: impl ToString) -> Self {
        Self {
            credentials: Some(Credentials::new(api_key.to_string(), secret.to_string())),
            ..self
        }
    }

    pub fn is_signed_in(&self) -> bool {
        self.credentials.is_some()
    }

    pub async fn create_order(
        &self,
        symbol: ContractSymbol,
        quantity: i32,
        side: Side,
        text: Option<String>,
    ) -> Result<Order> {
        let order = self
            .send_request(PostOrderRequest {
                symbol,
                side: Some(side),
                order_qty: Some(quantity),
                ord_type: Some(OrdType::Market),
                text,
            })
            .await?;
        Ok(order)
    }

    /// Retrieve the position information for all contract symbols.
    pub async fn positions(&self) -> Result<Vec<Position>> {
        let positions = self.send_request(GetPositionRequest).await?;
        Ok(positions)
    }

    async fn send_request<R>(&self, req: R) -> Result<R::Response>
    where
        R: Request,
        R::Response: DeserializeOwned,
    {
        let url = format!("{}{}", self.url, R::ENDPOINT);
        let mut url = Url::parse(&url)?;

        if matches!(R::METHOD, Method::GET | Method::DELETE) && R::HAS_PAYLOAD {
            url.set_query(Some(&to_ustring(&req)?));
        }

        let body = match R::METHOD {
            Method::PUT | Method::POST => to_jstring(&req)?,
            _ => "".to_string(),
        };

        let mut builder = self.client.request(R::METHOD, url.clone());

        if R::SIGNED {
            let credentials = match &self.credentials {
                None => {
                    bail!("Bitmex client not signed in")
                }
                Some(credentials) => credentials,
            };

            let start = SystemTime::now();
            let expires = start
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .add(Duration::from_secs(5))
                .as_secs();
            let (key, signature) = credentials.signature(R::METHOD, expires, &url, &body);
            builder = builder
                .header("api-expires", expires)
                .header("api-key", key)
                .header("api-signature", signature)
        }

        let resp = builder
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?;

        let response = self.handle_response(resp).await?;

        Ok(response)
    }

    async fn handle_response<T: DeserializeOwned>(&self, resp: Response) -> Result<T> {
        let status = resp.status();
        let content = resp.text().await?;
        if status.is_success() {
            match from_str::<T>(&content) {
                Ok(ret) => Ok(ret),
                Err(e) => {
                    bail!("Cannot deserialize '{}'. '{}'", content, e);
                }
            }
        } else {
            match from_str::<BitMEXErrorResponse>(&content) {
                Ok(ret) => bail!("Bitmex error: {:?}", ret),
                Err(e) => {
                    bail!("Cannot deserialize error '{}'. '{}'", content, e);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Credentials {
    api_key: String,
    secret: String,
}

impl Credentials {
    fn new(api_key: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            secret: secret.into(),
        }
    }

    fn signature(&self, method: Method, expires: u64, url: &Url, body: &str) -> (&str, String) {
        // Signature: hex(HMAC_SHA256(apiSecret, verb + path + expires + data))
        let signed_key = hmac::Key::new(hmac::HMAC_SHA256, self.secret.as_bytes());
        let sign_message = match url.query() {
            Some(query) => format!(
                "{}{}?{}{}{}",
                method.as_str(),
                url.path(),
                query,
                expires,
                body
            ),
            None => format!("{}{}{}{}", method.as_str(), url.path(), expires, body),
        };

        let signature = hexify(hmac::sign(&signed_key, sign_message.as_bytes()));
        (self.api_key.as_str(), signature)
    }
}

#[cfg(test)]
mod test {
    use super::Credentials;
    use anyhow::Result;
    use reqwest::Method;
    use reqwest::Url;

    #[test]
    fn test_signature_get() -> Result<()> {
        let tr = Credentials::new(
            "LAqUlngMIQkIUjXMUreyu3qn",
            "chNOOS4KvNXR_Xq4k4c9qsfoKWvnDecLATCRlcBwyKDYnWgO",
        );
        let (_, sig) = tr.signature(
            Method::GET,
            1518064236,
            &Url::parse("http://a.com/api/v1/instrument")?,
            "",
        );
        assert_eq!(
            sig,
            "c7682d435d0cfe87c16098df34ef2eb5a549d4c5a3c2b1f0f77b8af73423bf00"
        );
        Ok(())
    }

    #[test]
    fn test_signature_get_param() -> Result<()> {
        let tr = Credentials::new(
            "LAqUlngMIQkIUjXMUreyu3qn",
            "chNOOS4KvNXR_Xq4k4c9qsfoKWvnDecLATCRlcBwyKDYnWgO",
        );
        let (_, sig) = tr.signature(
            Method::GET,
            1518064237,
            &Url::parse_with_params(
                "http://a.com/api/v1/instrument",
                &[("filter", r#"{"symbol": "XBTM15"}"#)],
            )?,
            "",
        );
        assert_eq!(
            sig,
            "e2f422547eecb5b3cb29ade2127e21b858b235b386bfa45e1c1756eb3383919f"
        );
        Ok(())
    }

    #[test]
    fn test_signature_post() -> Result<()> {
        let credentials = Credentials::new(
            "LAqUlngMIQkIUjXMUreyu3qn",
            "chNOOS4KvNXR_Xq4k4c9qsfoKWvnDecLATCRlcBwyKDYnWgO",
        );
        let (_, sig) = credentials.signature(
            Method::POST,
            1518064238,
            &Url::parse("http://a.com/api/v1/order")?,
            r#"{"symbol":"XBTM15","price":219.0,"clOrdID":"mm_bitmex_1a/oemUeQ4CAJZgP3fjHsA","orderQty":98}"#,
        );
        assert_eq!(
            sig,
            "1749cd2ccae4aa49048ae09f0b95110cee706e0944e6a14ad0b3a8cb45bd336b"
        );
        Ok(())
    }
}

// The error response from bitmex;
#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BitMEXErrorResponse {
    pub(crate) error: BitMEXErrorMessage,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BitMEXErrorMessage {
    pub(crate) message: String,
    pub(crate) name: String,
}
