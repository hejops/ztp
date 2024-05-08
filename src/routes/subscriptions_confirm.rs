use actix_web::web::Query;
use actix_web::HttpResponse;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

/// Failure to parse the request params will automatically return 400
#[tracing::instrument(name = "Confirming new subscriber", skip(_params))]
pub async fn confirm(_params: Query<Parameters>) -> HttpResponse { HttpResponse::Ok().finish() }
