use anyhow::bail;
use anyhow::Result;
use base64::engine::general_purpose;
use base64::Engine;
use rand::Rng;
use sha256::digest;

pub struct PreImage {
    pub pre_image: [u8; 32],
    pub hash: String,
}

impl PreImage {
    pub fn get_base64_encoded_pre_image(&self) -> String {
        general_purpose::URL_SAFE.encode(self.pre_image)
    }
}

pub fn create_pre_image() -> PreImage {
    let pre_image = inner_create_pre_image();
    let hash = inner_hash_pre_image(&pre_image);
    PreImage { pre_image, hash }
}

fn inner_create_pre_image() -> [u8; 32] {
    let mut rng = rand::thread_rng();
    let pre_image: [u8; 32] = rng.gen();

    pre_image
}

fn inner_hash_pre_image(pre_image: &[u8; 32]) -> String {
    digest(pre_image)
}

/// Converts the provided pre_image into a `[u8; 32]` and hashes it then.
///
/// Fails if `pre_image` is not a valid `[u8; 32]`
pub fn hash_pre_image_string(pre_image: &str) -> Result<String> {
    let pre_image = hex::decode(pre_image).unwrap();
    let pre_image: [u8; 32] = match pre_image.try_into() {
        Ok(array) => array,
        Err(_) => {
            bail!("Failed to hash pre_image string");
        }
    };
    Ok(digest(&pre_image))
}

#[cfg(test)]
pub mod tests {
    use crate::commons::pre_image::hash_pre_image_string;
    use crate::commons::pre_image::inner_hash_pre_image;

    #[test]
    pub fn given_preimage_computes_deterministic_hash() {
        let pre_image = "92b1de6841db0cf46cc40be6fe80110a0264513ab27eb822ed71ca517ffe8fd9";
        let pre_image = hex::decode(pre_image).unwrap();
        let pre_image: [u8; 32] = pre_image
            .try_into()
            .expect("Failed to convert Vec<u8> to [u8; 32]");

        let hash = inner_hash_pre_image(&pre_image);
        assert_eq!(
            hash,
            "75aeb75aeaf351089bbeed0e2c294915ab73bd3de4b990eb7029b9b65d1b1018"
        )
    }

    #[test]
    pub fn given_preimage_computes_deterministic_hash_from_string() {
        let pre_image = "92b1de6841db0cf46cc40be6fe80110a0264513ab27eb822ed71ca517ffe8fd9";

        let hash = hash_pre_image_string(pre_image).unwrap();
        assert_eq!(
            hash,
            "75aeb75aeaf351089bbeed0e2c294915ab73bd3de4b990eb7029b9b65d1b1018"
        )
    }
}
