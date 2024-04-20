use actix_web::HttpResponse;

/// Note: viewing http response requires `curl -v`
// async fn health_check() -> impl Responder { HttpResponse::Ok() }
pub async fn health_check() -> HttpResponse { HttpResponse::Ok().finish() }
