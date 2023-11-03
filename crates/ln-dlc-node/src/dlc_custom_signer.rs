//! This file has temporarily been copied from `https://github.com/p2pderivatives/rust-dlc/pull/97`.
//! We should reimplement some of these traits for production.

use crate::ln_dlc_wallet::LnDlcWallet;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::TxOut;
use dlc_manager::subchannel::LnDlcChannelSigner;
use dlc_manager::subchannel::LnDlcSignerProvider;
use lightning::ln::chan_utils::ChannelPublicKeys;
use lightning::ln::msgs::DecodeError;
use lightning::ln::script::ShutdownScript;
use lightning::sign::ChannelSigner;
use lightning::sign::EcdsaChannelSigner;
use lightning::sign::EntropySource;
use lightning::sign::InMemorySigner;
use lightning::sign::KeyMaterial;
use lightning::sign::KeysManager;
use lightning::sign::NodeSigner;
use lightning::sign::Recipient;
use lightning::sign::SignerProvider;
use lightning::sign::SpendableOutputDescriptor;
use lightning::sign::WriteableEcdsaChannelSigner;
use lightning::util::ser::Writeable;
use parking_lot::Mutex;
use parking_lot::MutexGuard;
use secp256k1_zkp::ecdsa::RecoverableSignature;
use secp256k1_zkp::Secp256k1;
use secp256k1_zkp::SecretKey;
use secp256k1_zkp::Signing;
use std::sync::Arc;

pub struct CustomSigner {
    in_memory_signer: Arc<Mutex<InMemorySigner>>,
    // TODO(tibo): this might not be safe.
    channel_public_keys: ChannelPublicKeys,
}

impl CustomSigner {
    pub fn new(in_memory_signer: InMemorySigner) -> Self {
        Self {
            channel_public_keys: in_memory_signer.pubkeys().clone(),
            in_memory_signer: Arc::new(Mutex::new(in_memory_signer)),
        }
    }

    fn in_memory_signer_lock(&self) -> MutexGuard<InMemorySigner> {
        self.in_memory_signer.lock()
    }
}

impl Clone for CustomSigner {
    fn clone(&self) -> Self {
        Self {
            in_memory_signer: self.in_memory_signer.clone(),
            channel_public_keys: self.channel_public_keys.clone(),
        }
    }
}

impl EcdsaChannelSigner for CustomSigner {
    fn sign_counterparty_commitment(
        &self,
        commitment_tx: &lightning::ln::chan_utils::CommitmentTransaction,
        preimages: Vec<lightning::ln::PaymentPreimage>,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<
        (
            secp256k1_zkp::ecdsa::Signature,
            Vec<secp256k1_zkp::ecdsa::Signature>,
        ),
        (),
    > {
        self.in_memory_signer_lock().sign_counterparty_commitment(
            commitment_tx,
            preimages,
            secp_ctx,
        )
    }

    fn validate_counterparty_revocation(&self, idx: u64, secret: &SecretKey) -> Result<(), ()> {
        self.in_memory_signer_lock()
            .validate_counterparty_revocation(idx, secret)
    }

    fn sign_holder_commitment_and_htlcs(
        &self,
        commitment_tx: &lightning::ln::chan_utils::HolderCommitmentTransaction,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<
        (
            secp256k1_zkp::ecdsa::Signature,
            Vec<secp256k1_zkp::ecdsa::Signature>,
        ),
        (),
    > {
        self.in_memory_signer_lock()
            .sign_holder_commitment_and_htlcs(commitment_tx, secp_ctx)
    }

    fn sign_justice_revoked_output(
        &self,
        justice_tx: &Transaction,
        input: usize,
        amount: u64,
        per_commitment_key: &SecretKey,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<secp256k1_zkp::ecdsa::Signature, ()> {
        self.in_memory_signer_lock().sign_justice_revoked_output(
            justice_tx,
            input,
            amount,
            per_commitment_key,
            secp_ctx,
        )
    }

    fn sign_justice_revoked_htlc(
        &self,
        justice_tx: &Transaction,
        input: usize,
        amount: u64,
        per_commitment_key: &SecretKey,
        htlc: &lightning::ln::chan_utils::HTLCOutputInCommitment,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<secp256k1_zkp::ecdsa::Signature, ()> {
        self.in_memory_signer_lock().sign_justice_revoked_htlc(
            justice_tx,
            input,
            amount,
            per_commitment_key,
            htlc,
            secp_ctx,
        )
    }

    fn sign_holder_htlc_transaction(
        &self,
        htlc_tx: &Transaction,
        input: usize,
        htlc_descriptor: &lightning::events::bump_transaction::HTLCDescriptor,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<secp256k1_zkp::ecdsa::Signature, ()> {
        self.in_memory_signer_lock().sign_holder_htlc_transaction(
            htlc_tx,
            input,
            htlc_descriptor,
            secp_ctx,
        )
    }

    fn sign_counterparty_htlc_transaction(
        &self,
        htlc_tx: &Transaction,
        input: usize,
        amount: u64,
        per_commitment_point: &secp256k1_zkp::PublicKey,
        htlc: &lightning::ln::chan_utils::HTLCOutputInCommitment,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<secp256k1_zkp::ecdsa::Signature, ()> {
        self.in_memory_signer_lock()
            .sign_counterparty_htlc_transaction(
                htlc_tx,
                input,
                amount,
                per_commitment_point,
                htlc,
                secp_ctx,
            )
    }

    fn sign_closing_transaction(
        &self,
        closing_tx: &lightning::ln::chan_utils::ClosingTransaction,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<secp256k1_zkp::ecdsa::Signature, ()> {
        self.in_memory_signer_lock()
            .sign_closing_transaction(closing_tx, secp_ctx)
    }

    fn sign_holder_anchor_input(
        &self,
        anchor_tx: &Transaction,
        input: usize,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<secp256k1_zkp::ecdsa::Signature, ()> {
        self.in_memory_signer_lock()
            .sign_holder_anchor_input(anchor_tx, input, secp_ctx)
    }

    fn sign_channel_announcement_with_funding_key(
        &self,
        msg: &lightning::ln::msgs::UnsignedChannelAnnouncement,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<secp256k1_zkp::ecdsa::Signature, ()> {
        self.in_memory_signer_lock()
            .sign_channel_announcement_with_funding_key(msg, secp_ctx)
    }
}

impl ChannelSigner for CustomSigner {
    fn get_per_commitment_point(
        &self,
        idx: u64,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> secp256k1_zkp::PublicKey {
        self.in_memory_signer_lock()
            .get_per_commitment_point(idx, secp_ctx)
    }

    fn release_commitment_secret(&self, idx: u64) -> [u8; 32] {
        self.in_memory_signer_lock().release_commitment_secret(idx)
    }

    fn validate_holder_commitment(
        &self,
        holder_tx: &lightning::ln::chan_utils::HolderCommitmentTransaction,
        preimages: Vec<lightning::ln::PaymentPreimage>,
    ) -> Result<(), ()> {
        self.in_memory_signer_lock()
            .validate_holder_commitment(holder_tx, preimages)
    }

    fn pubkeys(&self) -> &ChannelPublicKeys {
        &self.channel_public_keys
    }

    fn channel_keys_id(&self) -> [u8; 32] {
        self.in_memory_signer_lock().channel_keys_id()
    }

    fn provide_channel_parameters(
        &mut self,
        channel_parameters: &lightning::ln::chan_utils::ChannelTransactionParameters,
    ) {
        self.in_memory_signer_lock()
            .provide_channel_parameters(channel_parameters);
    }

    fn set_channel_value_satoshis(&mut self, value: u64) {
        self.in_memory_signer_lock()
            .set_channel_value_satoshis(value)
    }
}

impl LnDlcChannelSigner for CustomSigner {
    fn get_holder_split_tx_signature(
        &self,
        secp: &Secp256k1<secp256k1_zkp::All>,
        split_tx: &Transaction,
        original_funding_redeemscript: &Script,
        original_channel_value_satoshis: u64,
    ) -> std::result::Result<secp256k1_zkp::ecdsa::Signature, dlc_manager::error::Error> {
        dlc::util::get_raw_sig_for_tx_input(
            secp,
            split_tx,
            0,
            original_funding_redeemscript,
            original_channel_value_satoshis,
            &self.in_memory_signer_lock().funding_key,
        )
        .map_err(|e| e.into())
    }

    fn get_holder_split_tx_adaptor_signature(
        &self,
        secp: &Secp256k1<secp256k1_zkp::All>,
        split_tx: &Transaction,
        original_channel_value_satoshis: u64,
        original_funding_redeemscript: &Script,
        other_publish_key: &secp256k1_zkp::PublicKey,
    ) -> std::result::Result<secp256k1_zkp::EcdsaAdaptorSignature, dlc_manager::error::Error> {
        dlc::channel::get_tx_adaptor_signature(
            secp,
            split_tx,
            original_channel_value_satoshis,
            original_funding_redeemscript,
            &self.in_memory_signer_lock().funding_key,
            other_publish_key,
        )
        .map_err(|e| e.into())
    }
}

impl Writeable for CustomSigner {
    fn write<W: lightning::util::ser::Writer>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        self.in_memory_signer_lock().write(writer)
    }
}

pub struct CustomKeysManager {
    keys_manager: KeysManager,
    wallet: Arc<LnDlcWallet>,
}

impl CustomKeysManager {
    pub fn new(keys_manager: KeysManager, wallet: Arc<LnDlcWallet>) -> Self {
        Self {
            keys_manager,
            wallet,
        }
    }

    pub fn get_node_secret_key(&self) -> SecretKey {
        self.keys_manager.get_node_secret_key()
    }
}

impl CustomKeysManager {
    #[allow(clippy::result_unit_err)]
    pub fn spend_spendable_outputs<C: Signing>(
        &self,
        descriptors: &[&SpendableOutputDescriptor],
        outputs: Vec<TxOut>,
        change_destination_script: Script,
        feerate_sat_per_1000_weight: u32,
        secp_ctx: &Secp256k1<C>,
    ) -> Result<Transaction> {
        self.keys_manager
            .spend_spendable_outputs(
                descriptors,
                outputs,
                change_destination_script,
                feerate_sat_per_1000_weight,
                None,
                secp_ctx,
            )
            .map_err(|_| anyhow!("Could not spend spendable outputs"))
    }
}

impl LnDlcSignerProvider<CustomSigner> for CustomKeysManager {
    fn derive_ln_dlc_channel_signer(
        &self,
        channel_value_satoshis: u64,
        channel_keys_id: [u8; 32],
    ) -> CustomSigner {
        self.derive_channel_signer(channel_value_satoshis, channel_keys_id)
    }
}

impl SignerProvider for CustomKeysManager {
    type Signer = CustomSigner;

    fn get_destination_script(&self) -> Result<Script, ()> {
        let address = self.wallet.unused_address();
        Ok(address.script_pubkey())
    }

    fn get_shutdown_scriptpubkey(&self) -> std::result::Result<ShutdownScript, ()> {
        let address = self.wallet.unused_address();
        match address.payload {
            bitcoin::util::address::Payload::WitnessProgram { version, program } => {
                ShutdownScript::new_witness_program(version, &program)
                    .map_err(|_ignored| tracing::error!("Invalid shutdown script"))
            }
            _ => {
                tracing::error!("Tried to use a non-witness address. This must not ever happen.");
                Err(())
            }
        }
    }

    fn read_chan_signer(&self, reader: &[u8]) -> Result<Self::Signer, DecodeError> {
        let in_memory = self.keys_manager.read_chan_signer(reader)?;
        Ok(CustomSigner::new(in_memory))
    }

    fn generate_channel_keys_id(
        &self,
        inbound: bool,
        channel_value_satoshis: u64,
        user_channel_id: u128,
    ) -> [u8; 32] {
        self.keys_manager
            .generate_channel_keys_id(inbound, channel_value_satoshis, user_channel_id)
    }

    fn derive_channel_signer(
        &self,
        channel_value_satoshis: u64,
        channel_keys_id: [u8; 32],
    ) -> Self::Signer {
        let inner = self
            .keys_manager
            .derive_channel_signer(channel_value_satoshis, channel_keys_id);
        let pubkeys = inner.pubkeys();

        CustomSigner {
            channel_public_keys: pubkeys.clone(),
            in_memory_signer: Arc::new(Mutex::new(inner)),
        }
    }
}

impl NodeSigner for CustomKeysManager {
    fn get_inbound_payment_key_material(&self) -> KeyMaterial {
        self.keys_manager.get_inbound_payment_key_material()
    }

    fn get_node_id(&self, recipient: Recipient) -> Result<secp256k1_zkp::PublicKey, ()> {
        self.keys_manager.get_node_id(recipient)
    }

    fn ecdh(
        &self,
        recipient: Recipient,
        other_key: &secp256k1_zkp::PublicKey,
        tweak: Option<&secp256k1_zkp::Scalar>,
    ) -> Result<secp256k1_zkp::ecdh::SharedSecret, ()> {
        self.keys_manager.ecdh(recipient, other_key, tweak)
    }

    fn sign_invoice(
        &self,
        hrp_bytes: &[u8],
        invoice_data: &[bitcoin::bech32::u5],
        recipient: Recipient,
    ) -> Result<RecoverableSignature, ()> {
        self.keys_manager
            .sign_invoice(hrp_bytes, invoice_data, recipient)
    }

    fn sign_gossip_message(
        &self,
        msg: lightning::ln::msgs::UnsignedGossipMessage,
    ) -> Result<secp256k1_zkp::ecdsa::Signature, ()> {
        self.keys_manager.sign_gossip_message(msg)
    }
}

impl EntropySource for CustomKeysManager {
    fn get_secure_random_bytes(&self) -> [u8; 32] {
        self.keys_manager.get_secure_random_bytes()
    }
}

impl WriteableEcdsaChannelSigner for CustomSigner {}
