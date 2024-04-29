use crate::commons::reqwest_client;
use crate::config;
use crate::state::get_node;
use crate::state::get_or_create_tokio_runtime;
use reqwest::Url;

pub fn report_error_to_coordinator<E: ToString>(error: &E) {
    let client = reqwest_client();
    let pk = get_node().inner.info.pubkey;

    let url = Url::parse(&format!("http://{}", config::get_http_endpoint()))
        .expect("valid URL")
        .join("/api/report-error")
        .expect("valid URL");

    let error_string = error.to_string();

    match get_or_create_tokio_runtime() {
        Ok(runtime) => {
            runtime.spawn(async move {
                if let Err(e) = client
                    .post(url)
                    .json(&xxi_node::commons::ReportedError {
                        trader_pk: pk,
                        msg: error_string,
                    })
                    .send()
                    .await
                {
                    tracing::error!("Failed to report error to coordinator: {e}");
                }
            });
        }
        Err(e) => {
            tracing::error!("Failed to report error to coordinator, missing runtime: {e}");
        }
    }
}
