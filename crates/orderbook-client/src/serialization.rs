use crate::Signature;
use serde::de;
use serde::de::Unexpected;
use serde::de::Visitor;
use serde::ser::SerializeTuple;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut tup = serializer.serialize_tuple(3)?;
        tup.serialize_element(&self.pubkey.to_string())?;
        tup.serialize_element(&self.signature.to_string())?;
        tup.end()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DesserializationError {
    Read,
    PublicKey,
    Signature,
}

impl de::Expected for DesserializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DesserializationError::Read => formatter.write_str("Read error"),
            DesserializationError::PublicKey => formatter.write_str("Invalid public key"),
            DesserializationError::Signature => formatter.write_str("Invalid signature"),
        }
    }
}

impl fmt::Display for DesserializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DesserializationError::Read => formatter.write_str("Read error"),
            DesserializationError::PublicKey => formatter.write_str("Invalid public key"),
            DesserializationError::Signature => formatter.write_str("Invalid signature"),
        }
    }
}

impl std::error::Error for DesserializationError {}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SignatureVisitor;
        impl<'de> Visitor<'de> for SignatureVisitor {
            type Value = Signature;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid String")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Signature, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let pubkey =
                    seq.next_element::<String>()?
                        .ok_or(serde::de::Error::invalid_value(
                            Unexpected::Seq,
                            &DesserializationError::Read,
                        ))?;
                let pubkey = secp256k1_zkp::PublicKey::from_str(pubkey.as_str()).map_err(|_| {
                    de::Error::invalid_value(Unexpected::Seq, &DesserializationError::PublicKey)
                })?;

                let sig = seq
                    .next_element::<String>()?
                    .ok_or(serde::de::Error::invalid_value(
                        Unexpected::Seq,
                        &DesserializationError::Read,
                    ))?;

                let signature =
                    secp256k1_zkp::ecdsa::Signature::from_str(sig.as_str()).map_err(|_| {
                        de::Error::invalid_value(Unexpected::Seq, &DesserializationError::Signature)
                    })?;

                Ok(Signature { pubkey, signature })
            }
        }

        deserializer.deserialize_tuple(3, SignatureVisitor)
    }
}

#[cfg(test)]
mod test {
    use crate::Signature;
    use secp256k1_zkp::PublicKey;
    use secp256k1_zkp::SecretKey;
    use secp256k1_zkp::SECP256K1;
    use std::str::FromStr;

    #[test]
    fn test_serialize_signature() {
        let secret_key = SecretKey::from_slice(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 27, 29, 30, 31,
        ])
        .unwrap();
        let sig = Signature {
            pubkey: secret_key.public_key(SECP256K1),
            signature: "3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1".parse().unwrap(),
        };

        let serialized = serde_json::to_string(&sig).unwrap();

        assert_eq!(
            serialized,
            r#"["02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655","3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1"]"#
        );
    }

    #[test]
    fn test_deserialize_signature() {
        let sig = r#"["02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655","3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1"]"#;
        let serialized: Signature = serde_json::from_str(sig).unwrap();

        let signature = Signature {
            pubkey: PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655").unwrap(),
            signature: "3045022100ddd8e15dea994a3dd98c481d901fb46b7f3624bb25b4210ea10f8a00779c6f0e0220222235da47b1ba293184fa4a91b39999911c08020e069c9f4afa2d81586b23e1".parse().unwrap(),
        };

        assert_eq!(serialized, signature);
    }
}
