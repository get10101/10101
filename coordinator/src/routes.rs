use bitcoin::secp256k1::PublicKey;
use http_api_problem::HttpApiProblem;
use http_api_problem::StatusCode;
use ln_dlc_node::node::Node;
use rocket::serde::json::Json;
use rocket::State;
use std::sync::Arc;

#[rocket::get("/get_fake_scid/<target_node>")]
pub async fn get_fake_scid(
    node: &State<Arc<Node>>,
    target_node: String,
) -> Result<Json<u64>, HttpApiProblem> {
    let target_node: PublicKey = target_node.parse().map_err(|e| {
        HttpApiProblem::new(StatusCode::BAD_REQUEST)
            .title("Invalid public key")
            .detail(format!(
                "Provided public key {target_node} was not valid: {e:#}"
            ))
    })?;

    Ok(Json(node.create_intercept_scid(target_node)))
}
