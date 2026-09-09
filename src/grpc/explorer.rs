use bytes::Bytes;
use http::{Method, Request, Response, StatusCode, header};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Kasane gRPC-Web Explorer を配信する Tower Layer
#[derive(Clone)]
pub struct ExplorerLayer {
    html: &'static str,
}

impl ExplorerLayer {
    pub fn new(html: &'static str) -> Self {
        Self { html }
    }
}

impl<S> Layer<S> for ExplorerLayer {
    type Service = ExplorerService<S>;

    fn layer(&self, inner: S) -> ExplorerService<S> {
        ExplorerService {
            inner,
            html: self.html,
        }
    }
}

#[derive(Clone)]
pub struct ExplorerService<S> {
    inner: S,
    html: &'static str,
}

impl<S, ReqBody> Service<Request<ReqBody>> for ExplorerService<S>
where
    S: Service<Request<ReqBody>, Response = Response<tonic::body::Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = Response<tonic::body::Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response<tonic::body::Body>, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Pin<Box<dyn Future<Output = Result<Response<tonic::body::Body>, S::Error>> + Send>> {
        let is_get = req.method() == Method::GET;
        let path = req.uri().path();
        let is_explorer_path = path == "/" || path == "/explorer" || path == "/docs";

        if is_get && is_explorer_path {
            let body_bytes = Bytes::from_static(self.html.as_bytes());
            let body = tonic::body::Body::new(http_body_util::Full::new(body_bytes));

            let res = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(body)
                .unwrap();

            Box::pin(async move { Ok(res) })
        } else {
            let fut = self.inner.call(req);
            Box::pin(fut)
        }
    }
}
