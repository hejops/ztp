use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use sqlx::PgPool;

use crate::authentication::validate_credentials;
use crate::authentication::AuthError;
use crate::authentication::Credentials;
use crate::authentication::UserId;
use crate::routes::admin::dashboard::get_username;
use crate::utils::error_500;
use crate::utils::redirect;

#[derive(Deserialize)]
pub struct ChangePasswordFormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_repeat: Secret<String>,
}

/// `POST /admin/password`
pub async fn change_password(
    // remember: user-sent forms must be wrapped in `Form`, not `Data`!
    form: web::Form<ChangePasswordFormData>,
    // session: TypedSession,
    user_id: web::ReqData<UserId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // let user_id = reject_anonymous_users(session).await;
    let user_id = user_id.into_inner();

    if form.new_password.expose_secret() != form.new_password_repeat.expose_secret() {
        FlashMessage::error("The two passwords supplied do not match!").send();
        return Ok(redirect("/admin/password"));
    }

    if !(13..=128).contains(&form.new_password.expose_secret().len()) {
        FlashMessage::error("The new password is too short!").send();
        return Ok(redirect("/admin/password"));
    }

    let username = get_username(*user_id, &pool).await.map_err(error_500)?;

    let creds = Credentials {
        username,
        password: form.0.current_password,
    };

    if let Err(e) = validate_credentials(creds, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect!").send();
                Ok(redirect("/admin/password"))
            }
            AuthError::UnexpectedError(_) => Err(error_500(e)),
        };
    }

    crate::authentication::change_password(*user_id, form.0.new_password, &pool)
        .await
        .map_err(error_500)?;
    // TODO: should probably use info (not error), we only use error because of
    // change_password_form's filter
    FlashMessage::error("Password changed successfully.").send();
    Ok(redirect("/admin/password"))
}
