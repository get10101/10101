use bitcoin::secp256k1::PublicKey;
use dlc_manager::Wallet;
use http_api_problem::HttpApiProblem;
use http_api_problem::StatusCode;
use ln_dlc_node::node::Node;
use rocket::serde::json::Json;
use rocket::serde::Deserialize;
use rocket::serde::Serialize;
use rocket::State;
use std::sync::Arc;

#[rocket::post("/fake_scid/<target_node>")]
pub async fn post_fake_scid(
    node: &State<Arc<Node>>,
    target_node: String,
) -> Result<Json<u64>, HttpApiProblem> {
    let target_node: PublicKey = target_node.parse().map_err(|e| {
        HttpApiProblem::new(StatusCode::BAD_REQUEST).detail(format!(
            "Provided public key {target_node} was not valid: {e:#}"
        ))
    })?;

    Ok(Json(node.create_intercept_scid(target_node)))
}

#[rocket::get("/newaddress")]
pub async fn get_new_address(node: &State<Arc<Node>>) -> Result<Json<String>, HttpApiProblem> {
    let address = node.wallet.get_new_address().map_err(|e| {
        HttpApiProblem::new(StatusCode::INTERNAL_SERVER_ERROR)
            .detail(format!("Failed to get new address: {e:#}"))
    })?;
    Ok(Json(address.to_string()))
}

#[derive(Serialize, Deserialize)]
pub struct Balance {
    offchain: u64,
    onchain: u64,
}

#[rocket::get("/balance")]
pub async fn get_balance(node: &State<Arc<Node>>) -> Result<Json<Balance>, HttpApiProblem> {
    let offchain = node.get_ldk_balance();
    let onchain = node.get_on_chain_balance().map_err(|e| {
        HttpApiProblem::new(StatusCode::INTERNAL_SERVER_ERROR)
            .detail(format!("Failed to get balance: {e:#}"))
    })?;
    Ok(Json(Balance {
        offchain: offchain.available,
        onchain: onchain.confirmed,
    }))
}

#[rocket::get("/invoice")]
pub async fn get_invoice(node: &State<Arc<Node>>) -> Result<Json<String>, HttpApiProblem> {
    let invoice = node.create_invoice(2000).map_err(|e| {
        HttpApiProblem::new(StatusCode::INTERNAL_SERVER_ERROR)
            .detail(format!("Failed to create invoice: {e:#}"))
    })?;

    Ok(Json(invoice.to_string()))
}
