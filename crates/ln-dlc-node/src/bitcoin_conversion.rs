use bitcoin::consensus;
use bitcoin::hashes::Hash;
use bitcoin::psbt::PartiallySignedTransaction;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::Address;
use bitcoin::Block;
use bitcoin::BlockHash;
use bitcoin::Network;
use bitcoin::OutPoint;
use bitcoin::ScriptBuf;
use bitcoin::Sequence;
use bitcoin::Transaction;
use bitcoin::TxIn;
use bitcoin::TxOut;
use bitcoin::Txid;
use bitcoin::Witness;
use std::str::FromStr;

pub fn to_tx_30(tx: bitcoin_old::Transaction) -> Transaction {
    let bytes = bitcoin_old::consensus::serialize(&tx);

    consensus::deserialize(&bytes).expect("valid conversion")
}

pub fn to_tx_29(tx: Transaction) -> bitcoin_old::Transaction {
    let bytes = consensus::serialize(&tx);

    bitcoin_old::consensus::deserialize(&bytes).expect("valid conversion")
}

pub fn to_txin_30(txin: bitcoin_old::TxIn) -> TxIn {
    let bitcoin_old::TxIn {
        previous_output: bitcoin_old::OutPoint { txid, vout },
        script_sig,
        sequence,
        witness,
    } = txin;

    let txid = to_txid_30(txid);
    let previous_output = OutPoint { txid, vout };

    let script_sig = to_script_30(script_sig);
    let sequence = Sequence(sequence.0);
    let witness = Witness::from_slice(&witness.to_vec());

    TxIn {
        previous_output,
        script_sig,
        sequence,
        witness,
    }
}

pub fn to_outpoint_30(outpoint: bitcoin_old::OutPoint) -> OutPoint {
    let txid = to_txid_30(outpoint.txid);

    OutPoint {
        txid,
        vout: outpoint.vout,
    }
}

pub fn to_outpoint_29(outpoint: OutPoint) -> bitcoin_old::OutPoint {
    let txid = to_txid_29(outpoint.txid);

    bitcoin_old::OutPoint {
        txid,
        vout: outpoint.vout,
    }
}

pub fn to_txout_30(txout: bitcoin_old::TxOut) -> TxOut {
    let value = txout.value;
    let script_pubkey = to_script_30(txout.script_pubkey);

    TxOut {
        value,
        script_pubkey,
    }
}

pub fn to_txout_29(txout: TxOut) -> bitcoin_old::TxOut {
    let value = txout.value;
    let script_pubkey = to_script_29(txout.script_pubkey);

    bitcoin_old::TxOut {
        value,
        script_pubkey,
    }
}

pub fn to_script_30(script: bitcoin_old::Script) -> ScriptBuf {
    ScriptBuf::from_bytes(script.to_bytes())
}

pub fn to_script_29(script: ScriptBuf) -> bitcoin_old::Script {
    bitcoin_old::Script::from(script.to_bytes())
}

pub fn to_txid_30(txid: bitcoin_old::Txid) -> Txid {
    Txid::from_slice(bitcoin_old::hashes::Hash::as_inner(&txid).as_slice())
        .expect("valid conversion")
}

pub fn to_txid_29(txid: Txid) -> bitcoin_old::Txid {
    bitcoin_old::hashes::Hash::from_slice(bitcoin::hashes::Hash::as_byte_array(&txid).as_slice())
        .expect("valid conversion")
}

pub fn to_address_29(address: Address) -> bitcoin_old::Address {
    let s = address.to_string();

    bitcoin_old::Address::from_str(&s).expect("valid address")
}

pub fn to_block_29(block: Block) -> bitcoin_old::Block {
    let bytes = consensus::serialize(&block);

    bitcoin_old::consensus::deserialize(&bytes).expect("valid conversion")
}

pub fn to_block_hash_30(block_hash: bitcoin_old::BlockHash) -> BlockHash {
    Hash::from_slice(bitcoin_old::hashes::Hash::as_inner(&block_hash)).expect("valid conversion")
}

pub fn to_block_hash_29(block_hash: BlockHash) -> bitcoin_old::BlockHash {
    bitcoin_old::hashes::Hash::from_slice(Hash::as_byte_array(&block_hash))
        .expect("valid conversion")
}

pub fn to_network_29(network: Network) -> bitcoin_old::Network {
    match network {
        Network::Bitcoin => bitcoin_old::Network::Bitcoin,
        Network::Testnet => bitcoin_old::Network::Testnet,
        Network::Signet => bitcoin_old::Network::Signet,
        Network::Regtest => bitcoin_old::Network::Regtest,
        _ => unreachable!(),
    }
}

pub fn to_psbt_30(
    psbt: bitcoin_old::psbt::PartiallySignedTransaction,
) -> PartiallySignedTransaction {
    let bytes = bitcoin_old::consensus::serialize(&psbt);

    PartiallySignedTransaction::deserialize(&bytes).expect("valid conversion")
}

pub fn to_psbt_29(
    psbt: PartiallySignedTransaction,
) -> bitcoin_old::psbt::PartiallySignedTransaction {
    let bytes = psbt.serialize();

    bitcoin_old::consensus::deserialize(&bytes).expect("valid conversion")
}

pub fn to_secp_pk_30(pk: bitcoin_old::secp256k1::PublicKey) -> bitcoin::secp256k1::PublicKey {
    let pk = pk.serialize();
    bitcoin::secp256k1::PublicKey::from_slice(&pk).expect("valid conversion")
}

pub fn to_secp_pk_29(pk: bitcoin::secp256k1::PublicKey) -> bitcoin_old::secp256k1::PublicKey {
    let pk = pk.serialize();
    bitcoin_old::secp256k1::PublicKey::from_slice(&pk).expect("valid conversion")
}

pub fn to_xonly_pk_30(pk: bitcoin_old::XOnlyPublicKey) -> bitcoin::secp256k1::XOnlyPublicKey {
    let pk = pk.serialize();
    bitcoin::secp256k1::XOnlyPublicKey::from_slice(&pk).expect("valid conversion")
}

pub fn to_xonly_pk_29(pk: bitcoin::secp256k1::XOnlyPublicKey) -> bitcoin_old::XOnlyPublicKey {
    let pk = pk.serialize();
    bitcoin_old::XOnlyPublicKey::from_slice(&pk).expect("valid conversion")
}

pub fn to_secp_sk_30(sk: bitcoin_old::secp256k1::SecretKey) -> bitcoin::secp256k1::SecretKey {
    let sk = sk.secret_bytes();
    bitcoin::secp256k1::SecretKey::from_slice(&sk).expect("valid conversion")
}

pub fn to_ecdsa_signature_30(signature: bitcoin_old::secp256k1::ecdsa::Signature) -> Signature {
    let sig = signature.serialize_compact();
    Signature::from_compact(&sig).expect("valid conversion")
}

pub fn to_ecdsa_signature_29(signature: Signature) -> bitcoin_old::secp256k1::ecdsa::Signature {
    let sig = signature.serialize_compact();
    bitcoin_old::secp256k1::ecdsa::Signature::from_compact(&sig).expect("valid conversion")
}
