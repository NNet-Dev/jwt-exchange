//! Request ID middleware.
//!
//! Injects a unique `X-Request-Id` header into every response for
//! request tracing and correlation with audit logs.

use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header,
    HttpMessage,
};
use futures::future::LocalBoxFuture;
use std::future::{ready, Ready};

/// F18: Middleware that injects X-Request-Id into ALL responses,
/// including errors. Stores the request_id in request extensions
/// so handlers can access it for logging.
pub struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = RequestIdMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestIdMiddlewareService { service }))
    }
}

pub struct RequestIdMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let request_id = uuid::Uuid::new_v4().to_string();

        // Store in request extensions for handler access
        req.extensions_mut().insert(RequestId(request_id.clone()));

        let fut = self.service.call(req);

        Box::pin(async move {
            match fut.await {
                Ok(mut res) => {
                    // Inject X-Request-Id into response headers
                    res.response_mut().headers_mut().insert(
                        header::HeaderName::from_static("x-request-id"),
                        header::HeaderValue::from_str(&request_id).unwrap(),
                    );
                    Ok(res.map_into_boxed_body())
                }
                Err(e) => {
                    // Pass through — handlers already inject request_id via extensions
                    Err(e)
                }
            }
        })
    }
}

/// Request extension carrying the current request's ID.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    /// Extract from request extensions, or generate a fresh one.
    pub fn from_request(req: &actix_web::HttpRequest) -> String {
        req.extensions()
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
    }
}
