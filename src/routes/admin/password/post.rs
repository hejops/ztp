use actix_web::web;
use actix_web::HttpResponse;
use secrecy::Secret;
use serde::Deserialize;

use crate::session_state::TypedSession;
use crate::utils::error_500;
use crate::utils::redirect;

#[derive(Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_repeat: Secret<String>,
}

/// `POST /admin/password`
pub async fn change_password(
    // remember: user-sent forms must be wrapped in `Form`, not `Data`!
    form: web::Form<FormData>,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    if session.get_user_id().map_err(error_500)?.is_none() {
        return Ok(redirect("/login"));
    };
    todo!()
}
