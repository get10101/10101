use std::path::Path;

use anyhow::bail;
use anyhow::Result;
use bdk::bitcoin;
use bdk::bitcoin::util::bip32::ExtendedPrivKey;
use bip39::Language;
use bip39::Mnemonic;
use bitcoin::Network;
use hkdf::Hkdf;
use sha2::Sha256;

#[derive(Clone)]
pub struct Bip39Seed {
    mnemonic: Mnemonic,
}

impl Bip39Seed {
    pub fn new() -> Result<Self> {
        let mut rng = rand::thread_rng();

        let word_count = 12;
        let mnemonic = Mnemonic::generate_in_with(&mut rng, Language::English, word_count)?;

        Ok(Self { mnemonic })
    }

    /// Initialise a [`Seed`] from a path.
    /// Generates new seed if there was no seed found in the given path
    pub fn initialize(seed_file: &Path) -> Result<Self> {
        let seed = if !seed_file.exists() {
            tracing::info!("No seed found. Generating new seed");
            let seed = Self::new()?;
            seed.write_to(seed_file)?;
            seed
        } else {
            Bip39Seed::read_from(seed_file)?
        };
        Ok(seed)
    }

    fn seed(&self) -> [u8; 64] {
        // passing an empty string here is the expected argument if the seed should not be
        // additionally password protected (according to https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki#from-mnemonic-to-seed)
        self.mnemonic.to_seed_normalized("TREZOR")
    }

    pub fn lightning_seed(&self) -> LightningSeed {
        let mut seed = [0u8; 32];

        Hkdf::<Sha256>::new(None, &self.seed())
            .expand(b"LIGHTNING_WALLET_SEED", &mut seed)
            .expect("array is of correct length");
        seed
    }

    pub fn wallet_seed(&self) -> WalletSeed {
        let mut ext_priv_key_seed = [0u8; 64];

        Hkdf::<Sha256>::new(None, &self.seed())
            .expand(b"BITCOIN_WALLET_SEED", &mut ext_priv_key_seed)
            .expect("array is of correct length");

        WalletSeed {
            seed: ext_priv_key_seed,
        }
    }

    pub fn get_seed_phrase(&self) -> Vec<String> {
        self.mnemonic.word_iter().map(|word| word.into()).collect()
    }

    // Read the entropy used to generate Mnemonic from disk
    fn read_from(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;

        let seed: Bip39Seed = TryInto::try_into(bytes)?;
        Ok(seed)
    }

    // Store the entropy used to generate Mnemonic on disk
    fn write_to(&self, path: &Path) -> Result<()> {
        if path.exists() {
            let path = path.display();
            bail!("Refusing to overwrite file at {path}")
        }
        std::fs::write(path, self.mnemonic.to_entropy())?;

        Ok(())
    }
}

pub struct WalletSeed {
    seed: [u8; 64],
}

impl WalletSeed {
    pub fn derive_extended_priv_key(&self, network: Network) -> Result<ExtendedPrivKey> {
        let ext_priv_key = ExtendedPrivKey::new_master(network, &self.seed)?;
        Ok(ext_priv_key)
    }
}

impl TryFrom<Vec<u8>> for Bip39Seed {
    type Error = anyhow::Error;
    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        let mnemonic = Mnemonic::from_entropy(&bytes)?;
        Ok(Bip39Seed { mnemonic })
    }
}

impl From<Mnemonic> for Bip39Seed {
    fn from(mnemonic: Mnemonic) -> Self {
        Bip39Seed { mnemonic }
    }
}

pub type LightningSeed = [u8; 32];

#[cfg(test)]
mod tests {
    use bip39::Mnemonic;
    use bitcoin::util::bip32::ExtendedPrivKey;
    use bitcoin::Network;
    use std::env::temp_dir;

    use crate::seed::Bip39Seed;

    #[test]
    fn create_bip39_seed() {
        let seed = Bip39Seed::new().expect("seed to be generated");
        let phrase = seed.get_seed_phrase();
        assert_eq!(12, phrase.len());
    }

    #[test]
    fn reinitialised_seed_is_the_same() {
        let mut path = temp_dir();
        path.push("seed");
        let seed_1 = Bip39Seed::initialize(&path).unwrap();
        let seed_2 = Bip39Seed::initialize(&path).unwrap();
        assert_eq!(
            seed_1.mnemonic, seed_2.mnemonic,
            "Reinitialised wallet should contain the same mnemonic"
        );
        assert_eq!(
            seed_1.seed(),
            seed_2.seed(),
            "Seed derived from mnemonic should be the same"
        );
    }

    #[test]
    fn deterministic_seed() {
        let mnemonic = Mnemonic::parse(
            "rule segment glance broccoli glove seminar plunge element artist stock clown thank",
        )
        .unwrap();
        let seed = Bip39Seed::from(mnemonic);

        let wallet_seed = seed.seed();
        let ln_seed = seed.lightning_seed();
        assert_eq!(hex::encode(wallet_seed), "32ea66d60c979ec4392e6364ce3debc38823d33864dfdb31b8aef227ee60813b850be5af70a758d93e50faf9f8b9eecea0c7e928fad9a2edb6a2af1f8c1a2bfd");
        assert_eq!(
            hex::encode(ln_seed),
            "1cf21ab62bf5a5ee40896158cbbc18b9ad75805e1824a252d8060c6c075b228f"
        );
    }

    #[test]
    fn test_vector() {
        // taken from https://github.com/trezor/python-mnemonic/blob/master/vectors.json
        // note: all passphrases are `TREZOR` which was hardcoded at the top for the sake of this test

        let mnemonic = Mnemonic::parse(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
            .unwrap();
        let bip39seed = Bip39Seed::from(mnemonic);
        let seed = bip39seed.seed();
        assert_eq!(hex::encode(seed), "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04");

        // This is the bip39 approach of deriving an extended private key from the seed
        let xprv = ExtendedPrivKey::new_master(Network::Bitcoin, &seed).unwrap();
        assert_eq!(xprv.to_string(),"xprv9s21ZrQH143K3h3fDYiay8mocZ3afhfULfb5GX8kCBdno77K4HiA15Tg23wpbeF1pLfs1c5SPmYHrEpTuuRhxMwvKDwqdKiGJS9XFKzUsAF");

        // This is our approach, which is not compatible with the official bip39 test vectors, the assert at the end will fail
        let wallet_seed = bip39seed.wallet_seed();
        let key = wallet_seed
            .derive_extended_priv_key(Network::Bitcoin)
            .unwrap();
        assert_eq!(key.to_string(),"xprv9s21ZrQH143K3h3fDYiay8mocZ3afhfULfb5GX8kCBdno77K4HiA15Tg23wpbeF1pLfs1c5SPmYHrEpTuuRhxMwvKDwqdKiGJS9XFKzUsAF");
    }
}
