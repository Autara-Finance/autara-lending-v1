use axum::http::Response;
use axum::routing::get;
use axum::Router;
use prometheus::{Registry, TextEncoder};
use std::io;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

pub struct PrometheusExporter;

impl PrometheusExporter {
    pub async fn launch(
        metrics_addr: SocketAddr,
        runtime: Option<&tokio::runtime::Runtime>,
    ) -> io::Result<JoinHandle<()>> {
        let registry = prometheus::default_registry();
        let get_metrics_handler = || {
            let response = get_metrics_response(registry);
            async { response }
        };
        let metrics_app = Router::new().route("/metrics", get(get_metrics_handler));
        let metrics_listener = tokio::net::TcpListener::bind(metrics_addr).await?;
        if let Some(runtime) = runtime {
            return Ok(runtime.spawn(async move {
                axum::serve(metrics_listener, metrics_app)
                    .await
                    .expect("metrics server failed")
            }));
        } else {
            return Ok(tokio::spawn(async move {
                axum::serve(metrics_listener, metrics_app)
                    .await
                    .expect("metrics server failed")
            }));
        }
    }
}

const RESPONSE_CONTENT_TYPE: &str = "text/plain; version=0.0.4";
const CONTENT_TYPE: &str = "content-type";

fn get_metrics_response(registry: &Registry) -> Response<String> {
    let mut buffer = String::new();
    match TextEncoder::new().encode_utf8(&registry.gather(), &mut buffer) {
        Ok(()) => axum::http::Response::builder()
            .status(200)
            .header(CONTENT_TYPE, RESPONSE_CONTENT_TYPE)
            .body(buffer),
        Err(err) => axum::http::Response::builder()
            .status(500)
            .body(format!("{:?}", err)),
    }
    .expect("Error while building response")
}
