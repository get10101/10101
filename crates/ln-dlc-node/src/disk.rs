use crate::ln::TracingLogger;
use crate::NetworkGraph;
use bitcoin::Network;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::routing::scoring::ProbabilisticScoringParameters;
use lightning::util::ser::ReadableArgs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

pub(crate) fn persistent_scorer(
    path: &Path,
    graph: Arc<NetworkGraph>,
    logger: Arc<TracingLogger>,
) -> ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>> {
    let params = ProbabilisticScoringParameters::default();
    if let Ok(file) = File::open(path) {
        let args = (params.clone(), graph.clone(), logger.clone());
        match ProbabilisticScorer::read(&mut BufReader::new(file), args) {
            Ok(scorer) => return scorer,
            Err(e) => tracing::error!("Failed to read scorer from disk: {e}"),
        }
    }
    ProbabilisticScorer::new(params, graph, logger)
}

pub(crate) fn in_memory_scorer(
    _path: &Path,
    graph: Arc<NetworkGraph>,
    logger: Arc<TracingLogger>,
) -> ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>> {
    let params = ProbabilisticScoringParameters::default();
    ProbabilisticScorer::new(params, graph, logger)
}

pub(crate) fn read_network(
    path: &Path,
    network: Network,
    logger: Arc<TracingLogger>,
) -> NetworkGraph {
    if let Ok(file) = File::open(path) {
        match NetworkGraph::read(&mut BufReader::new(file), logger.clone()) {
            Ok(graph) => return graph,
            Err(e) => tracing::error!("Failed to read network graph from disk: {e}"),
        }
    }
    NetworkGraph::new(network, logger)
}
