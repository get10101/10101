use crate::api::Event;
use crate::api_lndlc::runtime;
use crate::api_lndlc::ELECTRS_ORIGIN;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::rand::thread_rng;
use bdk::bitcoin::secp256k1::rand::RngCore;
use bdk::bitcoin::Network;
use flutter_rust_bridge::StreamSink;
use ln_dlc_node::node::Node;
use ln_dlc_node::seed::Bip39Seed;
use std::net::TcpListener;
use std::path::Path;

pub fn run(stream: StreamSink<Event>, data_dir: String) -> Result<()> {
    let network = Network::Regtest;
    let runtime = runtime()?;
    runtime.block_on(async move {
        stream.add(Event::Init("Starting full ldk node".to_string()));
        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let data_dir = Path::new(&data_dir).join(network.to_string());
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)
                .context(format!("Could not create data dir for {network}"))?;
        }

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0").unwrap();
            listener.local_addr().expect("To get a free local address")
        };

        let seed_path = data_dir.join("seed");
        let seed = Bip39Seed::initialize(&seed_path)?;

        let node = Node::new_app(
            "coordinator".to_string(),
            network,
            data_dir.as_path(),
            address,
            ELECTRS_ORIGIN.to_string(),
            seed,
            ephemeral_randomness,
        )
        .await;

        let background_processor = node.start().await?;

        runtime.spawn_blocking(move || {
            // background processor joins on a sync thread, meaning that join here will block a
            // full thread, which is dis-encouraged to do in async code.
            if let Err(err) = background_processor.join() {
                tracing::error!(?err, "Background processor stopped unexpected");
            }
        });

        Ok(())
    })
}
