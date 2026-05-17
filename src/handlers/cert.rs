//! GET /api/v1/cert — returns the public X.509 certificate.
//!
//! Serves the PEM-formatted public certificate for the RSA key pair
//! used to sign minted JWTs, so downstream services can verify tokens.

use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::error::ServiceError;

pub async fn cert(state: web::Data<AppState>) -> Result<HttpResponse, ServiceError> {
    if state.public_cert.is_empty() {
        return Err(ServiceError::CertNotLoaded);
    }

    Ok(HttpResponse::Ok()
        .content_type("application/x-pem-file")
        .body(state.public_cert.clone()))
}
