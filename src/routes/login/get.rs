use actix_web::http::header::ContentType;
use actix_web::HttpResponse;

/// `GET` endpoint (`login_form`)
pub async fn login_form() -> HttpResponse {
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(include_str!("./login.html"))
}
