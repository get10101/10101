//! This file has temporarily been copied from `https://github.com/p2pderivatives/rust-dlc/pull/97`.
//! We should reimplement some of these traits for production.

use crate::ln_dlc_wallet::LnDlcWallet;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::Script;
use bitcoin::Transaction;
use bitcoin::TxOut;
use lightning::chain::keysinterface::BaseSign;
use lightning::chain::keysinterface::ExtraSign;
use lightning::chain::keysinterface::InMemorySigner;
use lightning::chain::keysinterface::KeyMaterial;
use lightning::chain::keysinterface::KeysInterface;
use lightning::chain::keysinterface::KeysManager;
use lightning::chain::keysinterface::Recipient;
use lightning::chain::keysinterface::Sign;
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::ln::chan_utils::ChannelPublicKeys;
use lightning::ln::msgs::DecodeError;
use lightning::ln::script::ShutdownScript;
use lightning::util::ser::Writeable;
use secp256k1_zkp::ecdsa::RecoverableSignature;
use secp256k1_zkp::Secp256k1;
use secp256k1_zkp::SecretKey;
use secp256k1_zkp::Signing;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

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
        self.in_memory_signer
            .lock()
            .expect("Mutex to not be poisoned")
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

impl BaseSign for CustomSigner {
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

    fn sign_channel_announcement(
        &self,
        msg: &lightning::ln::msgs::UnsignedChannelAnnouncement,
        secp_ctx: &Secp256k1<bitcoin::secp256k1::All>,
    ) -> Result<
        (
            secp256k1_zkp::ecdsa::Signature,
            secp256k1_zkp::ecdsa::Signature,
        ),
        (),
    > {
        self.in_memory_signer_lock()
            .sign_channel_announcement(msg, secp_ctx)
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

    fn provide_channel_parameters(
        &mut self,
        channel_parameters: &lightning::ln::chan_utils::ChannelTransactionParameters,
    ) {
        self.in_memory_signer_lock()
            .provide_channel_parameters(channel_parameters);
    }
}

impl ExtraSign for CustomSigner {
    fn sign_with_fund_key_callback<F>(&self, cb: &mut F)
    where
        F: FnMut(&SecretKey),
    {
        self.in_memory_signer_lock().sign_with_fund_key_callback(cb)
    }

    fn set_channel_value_satoshis(&mut self, value: u64) {
        self.in_memory_signer_lock()
            .set_channel_value_satoshis(value)
    }
}

impl Writeable for CustomSigner {
    fn write<W: lightning::util::ser::Writer>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        self.in_memory_signer_lock().write(writer)
    }
}

impl Sign for CustomSigner {}

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
                secp_ctx,
            )
            .map_err(|_| anyhow!("Could not spend spendable outputs"))
    }
}

impl KeysInterface for CustomKeysManager {
    type Signer = CustomSigner;

    fn get_node_secret(&self, recipient: Recipient) -> Result<SecretKey, ()> {
        self.keys_manager.get_node_secret(recipient)
    }

    fn get_inbound_payment_key_material(&self) -> KeyMaterial {
        self.keys_manager.get_inbound_payment_key_material()
    }

    fn get_destination_script(&self) -> Script {
        let address = self
            .wallet
            .get_last_unused_address()
            .expect("Failed to retrieve new address from wallet.");
        address.script_pubkey()
    }

    fn get_shutdown_scriptpubkey(&self) -> ShutdownScript {
        let address = self
            .wallet
            .get_last_unused_address()
            .expect("Failed to retrieve new address from wallet.");
        match address.payload {
            bitcoin::util::address::Payload::WitnessProgram { version, program } => {
                ShutdownScript::new_witness_program(version, &program)
                    .expect("Invalid shutdown script.")
            }
            _ => panic!("Tried to use a non-witness address. This must not ever happen."),
        }
    }

    fn get_secure_random_bytes(&self) -> [u8; 32] {
        self.keys_manager.get_secure_random_bytes()
    }

    fn read_chan_signer(&self, reader: &[u8]) -> Result<Self::Signer, DecodeError> {
        let in_memory = self.keys_manager.read_chan_signer(reader)?;
        Ok(CustomSigner::new(in_memory))
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

    fn ecdh(
        &self,
        recipient: Recipient,
        other_key: &secp256k1_zkp::PublicKey,
        tweak: Option<&secp256k1_zkp::Scalar>,
    ) -> Result<secp256k1_zkp::ecdh::SharedSecret, ()> {
        self.keys_manager.ecdh(recipient, other_key, tweak)
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
