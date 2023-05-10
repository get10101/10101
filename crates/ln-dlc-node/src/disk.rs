use crate::ln::TracingLogger;
use crate::NetworkGraph;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::routing::scoring::ProbabilisticScoringParameters;
use lightning::util::ser::ReadableArgs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

pub(crate) fn read_scorer(
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
