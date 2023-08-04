use crate::ln::TracingLogger;
use crate::NetworkGraph;
use bitcoin::Network;
use lightning::util::ser::ReadableArgs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

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
