use actix_web::http::header::ContentType;
use actix_web::HttpResponse;

/// `GET /home`
pub async fn home() -> HttpResponse {
    HttpResponse::Ok()
        // .finish()
        // path relative to this file (checked at compile time!)
        .content_type(ContentType::html())
        .body(include_str!("./home.html"))
}
