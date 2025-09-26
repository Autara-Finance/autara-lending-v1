use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use jsonrpsee::server::middleware::rpc::RpcServiceT;
use jsonrpsee::types::Request;
use jsonrpsee::MethodResponse;
use pin_project::pin_project;
use tracing::instrument::Instrumented;
use tracing::Instrument;

#[derive(Copy, Clone, Debug)]
pub struct AutaraTraceLayer;

impl<S> tower::Layer<S> for AutaraTraceLayer {
    type Service = RpcLogger<S>;

    fn layer(&self, service: S) -> Self::Service {
        RpcLogger { service }
    }
}

#[derive(Debug)]
pub struct RpcLogger<S> {
    service: S,
}

impl<'a, S> RpcServiceT<'a> for RpcLogger<S>
where
    S: RpcServiceT<'a>,
{
    type Future = Instrumented<ResponseFuture<S::Future>>;

    #[tracing::instrument(name = "method_call", skip_all, fields(id = request.id().as_str(), method = request.method_name(), trace_id = gen_trace_id()))]
    fn call(&self, request: Request<'a>) -> Self::Future {
        tracing::info!("Received request");
        ResponseFuture {
            fut: self.service.call(request),
            received_at: Instant::now(),
        }
        .in_current_span()
    }
}

/// Response future to log the response for a method call.
#[pin_project]
pub struct ResponseFuture<F> {
    #[pin]
    fut: F,
    received_at: Instant,
}

impl<F> std::fmt::Debug for ResponseFuture<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResponseFuture")
    }
}

impl<F: Future<Output = MethodResponse>> Future for ResponseFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let fut = self.project();
        let res = fut.fut.poll(cx);
        if let Poll::Ready(m) = &res {
            if m.is_error() {
                tracing::error!("Request failed: {:?}", m);
            } else {
                tracing::info!("Request completed in {:?}", fut.received_at.elapsed());
            }
        }
        res
    }
}

fn gen_trace_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
