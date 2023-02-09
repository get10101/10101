use crate::NetworkGraph;
use crate::TracingLogger;
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
        if let Ok(scorer) = ProbabilisticScorer::read(&mut BufReader::new(file), args) {
            return scorer;
        }
    }
    ProbabilisticScorer::new(params, graph, logger)
}
