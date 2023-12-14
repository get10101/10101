use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use lightning::routing::gossip::ReadOnlyNetworkGraph;
use lightning::routing::router::PaymentParameters;
use lightning::routing::router::RouteParameters;
use lightning::routing::router::ScorerAccountingForInFlightHtlcs;
use lightning::routing::scoring::ProbabilisticScoringFeeParameters;
use ln_dlc_node::node::Node;
use ln_dlc_node::node::Storage;
use ln_dlc_node::storage::TenTenOneStorage;
use rand::thread_rng;
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::spawn_blocking;

const MAX_AMOUNT_MSAT: u64 = 500_000_000;

const FINAL_CLTV_EXPIRY_DELTA: u32 = 144;

const MAX_PATH_COUNT: u8 = 1;

pub async fn send_payment_probes_regularly<
    S: TenTenOneStorage + 'static,
    N: Storage + Sync + Send + 'static,
>(
    node: Arc<Node<S, N>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        if let Err(e) = spawn_blocking({
            let node = node.clone();
            move || send_random_probe(&node)
        })
        .await
        .expect("task to complete")
        {
            tracing::error!("Failed to send payment probe: {e:#}");
        }
    }
}

fn send_random_probe<S: TenTenOneStorage + 'static, N: Storage + Sync + Send + 'static>(
    node: &Node<S, N>,
) -> Result<()> {
    let random_node = match random_node(node.network_graph.read_only())? {
        Some(node) => node,
        None => {
            tracing::warn!("No probe to send with empty network graph");
            return Ok(());
        }
    };

    let random_amount_msat = thread_rng().gen_range(0..MAX_AMOUNT_MSAT);

    send_probe(node, random_node, random_amount_msat);

    Ok(())
}

fn send_probe<S: TenTenOneStorage + 'static, N: Storage + Sync + Send + 'static>(
    node: &Node<S, N>,
    recipient: PublicKey,
    amount_msat: u64,
) {
    let channel_manager = &node.channel_manager;
    let scorer = &node.scorer;

    let usable_channels = channel_manager.list_usable_channels();
    let usable_channels = usable_channels.iter().collect::<Vec<_>>();

    let route_parameters = {
        let mut payment_params =
            PaymentParameters::from_node_id(recipient, FINAL_CLTV_EXPIRY_DELTA);
        payment_params.max_path_count = MAX_PATH_COUNT;

        RouteParameters::from_payment_params_and_value(payment_params, amount_msat)
    };

    let scorer = scorer.read().expect("to be able to acquire read lock");
    let in_flight_htlcs = channel_manager.compute_inflight_htlcs();
    let inflight_scorer = ScorerAccountingForInFlightHtlcs::new(&scorer, &in_flight_htlcs);

    let route_res = lightning::routing::router::find_route(
        &channel_manager.get_our_node_id(),
        &route_parameters,
        &node.network_graph,
        Some(&usable_channels),
        node.logger.clone(),
        &inflight_scorer,
        &ProbabilisticScoringFeeParameters::default(),
        &[32; 32],
    );
    if let Ok(route) = route_res {
        for path in route.paths {
            let _ = channel_manager.send_probe(path);
        }
    }
}

fn random_node(network_graph: ReadOnlyNetworkGraph) -> Result<Option<PublicKey>> {
    let nodes = network_graph.nodes();
    let n_nodes = nodes.len();

    if n_nodes == 0 {
        return Ok(None);
    }

    let random_index = thread_rng().gen_range(0..n_nodes);

    let random_node = nodes
        .unordered_iter()
        .nth(random_index)
        .expect("node at index")
        .0
        .as_pubkey()
        .context("Cannot convert node ID to public key")?;

    Ok(Some(random_node))
}
