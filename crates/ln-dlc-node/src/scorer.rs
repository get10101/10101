use crate::ln::TracingLogger;
use crate::NetworkGraph;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::routing::scoring::ProbabilisticScoringDecayParameters;
use lightning::util::ser::ReadableArgs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

/// A scorer that is persistent to disk
pub fn persistent_scorer(
    path: &Path,
    graph: Arc<NetworkGraph>,
    logger: Arc<TracingLogger>,
) -> ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>> {
    let params = ProbabilisticScoringDecayParameters::default();
    if let Ok(file) = File::open(path) {
        let args = (params, graph.clone(), logger.clone());
        match ProbabilisticScorer::read(&mut BufReader::new(file), args) {
            Ok(scorer) => return scorer,
            Err(e) => tracing::error!("Failed to read scorer from disk: {e}"),
        }
    }
    ProbabilisticScorer::new(params, graph, logger)
}

/// A scorer that is in-memory only
pub fn in_memory_scorer(
    _path: &Path,
    graph: Arc<NetworkGraph>,
    logger: Arc<TracingLogger>,
) -> ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>> {
    let params = ProbabilisticScoringDecayParameters::default();
    ProbabilisticScorer::new(params, graph, logger)
}
