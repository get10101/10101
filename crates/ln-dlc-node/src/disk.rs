use crate::NetworkGraph;
use crate::TracingLogger;
use bitcoin::BlockHash;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::routing::scoring::ProbabilisticScoringParameters;
use lightning::util::ser::ReadableArgs;
use lightning::util::ser::Writer;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

pub(crate) fn persist_channel_peer(path: &Path, peer_info: &str) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(format!("{}\n", peer_info).as_bytes())
}

// pub(crate) fn read_channel_peer_data(
//     path: &Path,
// ) -> Result<HashMap<PublicKey, SocketAddr>, std::io::Error> {
//     let mut peer_data = HashMap::new();
//     if !Path::new(&path).exists() {
//         return Ok(HashMap::new());
//     }
//     let file = File::open(path)?;
//     let reader = BufReader::new(file);
//     for line in reader.lines() {
//         match cli::parse_peer_info(line.unwrap()) {
//             Ok((pubkey, socket_addr)) => {
//                 peer_data.insert(pubkey, socket_addr);
//             }
//             Err(e) => return Err(e),
//         }
//     }
//     Ok(peer_data)
// }

pub(crate) fn read_network(
    path: &Path,
    genesis_hash: BlockHash,
    logger: Arc<TracingLogger>,
) -> NetworkGraph {
    if let Ok(file) = File::open(path) {
        if let Ok(graph) = NetworkGraph::read(&mut BufReader::new(file), logger.clone()) {
            return graph;
        }
    }
    NetworkGraph::new(genesis_hash, logger)
}

pub(crate) fn read_scorer(
    path: &Path,
    graph: Arc<NetworkGraph>,
    logger: Arc<TracingLogger>,
) -> ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>> {
    let params = ProbabilisticScoringParameters::default();
    if let Ok(file) = File::open(path) {
        let args = (params.clone(), Arc::clone(&graph), Arc::clone(&logger));
        if let Ok(scorer) = ProbabilisticScorer::read(&mut BufReader::new(file), args) {
            return scorer;
        }
    }
    ProbabilisticScorer::new(params, graph, logger)
}
