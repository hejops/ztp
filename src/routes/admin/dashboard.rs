use std::fmt::Debug;

use actix_session::Session;
use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::HttpResponse;
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::utils::error_500;
use crate::utils::redirect;

async fn get_username(
    user_id: Uuid,
    pool: &PgPool,
) -> anyhow::Result<String> {
    let row = sqlx::query!(
        "
        SELECT username FROM users 
        WHERE user_id = $1
",
        user_id
    )
    .fetch_one(pool)
    .await
    .context(format!("No user found with id {user_id}"))?;
    Ok(row.username)
}

/// `GET /admin/dashboard`
pub async fn admin_dashboard(
    session: Session,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let username = match session.get::<Uuid>("user_id").map_err(error_500)? {
        Some(user_id) => get_username(user_id, &pool).await.map_err(error_500)?,
        None => return Ok(redirect("/login")),
    };

    let body = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Admin dashboard</title>
</head>
<body>
    <p>Welcome {username}!</p>
</body>
</html>"#
    );

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}
