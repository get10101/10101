use bitcoin::secp256k1::Message as SecpMessage;
use bitcoin::secp256k1::PublicKey;
use secp256k1::Secp256k1;
use secp256k1::SecretKey;
use secp256k1::VerifyOnly;
use serde::Deserialize;
use serde::Serialize;
use sha2::digest::FixedOutput;
use sha2::Digest;
use sha2::Sha256;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignedValue<T> {
    pub value: T,
    /// A signature of the sha256 of [`value`]
    pub signature: secp256k1::ecdsa::Signature,
}

impl<T: Serialize> SignedValue<T> {
    pub fn new(value: T, secret_key: SecretKey) -> anyhow::Result<Self> {
        let serialized_value = serde_json::to_vec(&value)?;
        let message = create_sign_message(serialized_value);
        let signature = secret_key.sign_ecdsa(message);

        Ok(Self { value, signature })
    }

    pub fn verify(&self, secp: &Secp256k1<VerifyOnly>, node_id: &PublicKey) -> anyhow::Result<()> {
        let message = serde_json::to_vec(&self.value)?;
        let message = create_sign_message(message);

        secp.verify_ecdsa(&message, &self.signature, node_id)?;

        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub pubkey: PublicKey,
    pub signature: secp256k1::ecdsa::Signature,
}

pub fn create_sign_message(message: Vec<u8>) -> SecpMessage {
    let hashed_message = Sha256::new().chain_update(message).finalize_fixed();

    let msg = SecpMessage::from_slice(hashed_message.as_slice())
        .expect("The message is static, hence this should never happen");
    msg
}

#[cfg(test)]
mod test {
    use crate::commons::signature::Signature;
    use crate::commons::SignedValue;
    use bitcoin::secp256k1::PublicKey;
    use bitcoin::secp256k1::SecretKey;
    use secp256k1::Secp256k1;
    use secp256k1::SECP256K1;
    use serde::Deserialize;
    use serde::Serialize;
    use std::str::FromStr;

    fn dummy_public_key() -> PublicKey {
        PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
            .unwrap()
    }

    #[test]
    fn test_serialize_signature() {
        let secret_key = SecretKey::from_slice(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 27, 29, 30, 31,
        ])
        .unwrap();
        let sig = Signature {
            pubkey: secret_key.public_key(&secp256k1::Secp256k1::new()),
            signature: "3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1".parse().unwrap(),
        };

        let serialized = serde_json::to_string(&sig).unwrap();

        assert_eq!(
            serialized,
            r#"{"pubkey":"02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655","signature":"3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1"}"#
        );
    }

    #[test]
    fn test_deserialize_signature() {
        let sig = r#"{"pubkey":"02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655","signature":"3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1"}"#;
        let serialized: Signature = serde_json::from_str(sig).unwrap();

        let signature = Signature {
            pubkey: dummy_public_key(),
            signature: "3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1".parse().unwrap(),
        };

        assert_eq!(serialized, signature);
    }

    fn test_secret_key() -> SecretKey {
        SecretKey::from_slice(&[
            32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 27, 29, 30, 31,
        ])
        .unwrap()
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct DummyModel {
        dummy1: String,
        dummy2: u64,
    }

    #[test]
    fn test_signed_value_valid_signature() {
        let secret_key = test_secret_key();

        let value = DummyModel {
            dummy1: "10101".to_string(),
            dummy2: 10101,
        };

        let signed_value = SignedValue::new(value, secret_key).unwrap();

        signed_value
            .verify(
                &Secp256k1::verification_only(),
                &secret_key.public_key(SECP256K1),
            )
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "signature failed verification")]
    fn test_signed_value_invalid_signature() {
        let value = DummyModel {
            dummy1: "10101".to_string(),
            dummy2: 10101,
        };

        let signed_value = SignedValue::new(value.clone(), test_secret_key()).unwrap();

        signed_value
            .verify(&Secp256k1::verification_only(), &dummy_public_key())
            .unwrap();
    }
}
