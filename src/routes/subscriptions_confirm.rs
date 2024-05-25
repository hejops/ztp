use std::fmt::Debug;

use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::web::Query;
use actix_web::HttpResponse;
use actix_web::ResponseError;
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::error_chain_fmt;

#[derive(Deserialize)]
pub struct Parameters {
    /// 25-character alphanumeric, generated by `subscribe`
    subscription_token: String,
}

#[derive(thiserror::Error)]
pub enum ConfirmError {
    #[error("Token not found")]
    ValidationError,

    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for ConfirmError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        error_chain_fmt(self, f)?;
        Ok(())
    }
}

impl ResponseError for ConfirmError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::ValidationError => StatusCode::UNAUTHORIZED, // 400
            _ => StatusCode::INTERNAL_SERVER_ERROR,            // 500
        }
    }
}
/// Fails if `token` not found in `subscription_tokens` table. The `id` returned
/// may be empty, so this should be checked by the caller.
#[tracing::instrument(name = "Getting id of new subscriber", skip(pool, token))]
async fn get_subscriber_id_from_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    // "What happens if the subscription token is well-formatted but non-existent
    // [in the db]?" -- what does 'well-formatted' mean? how can it be
    // non-existent?

    let id = sqlx::query!(
        "
    SELECT subscriber_id FROM subscription_tokens
    WHERE subscription_token = $1
",
        token,
    )
    .fetch_optional(pool)
    .await?
    // .map_err(|e| {
    //     tracing::error!("bad query: {e:?}");
    //     e
    // })
    .map(|u| u.subscriber_id);
    Ok(id)
}

/// Idempotent
#[tracing::instrument(name = "UPDATEing status of new subscriber", skip(pool))]
async fn confirm_subscriber(
    pool: &PgPool,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "
    UPDATE subscriptions SET status = 'confirmed'
    WHERE id = $1
",
        id,
    )
    .execute(pool)
    .await
    // .map_err(|e| {
    //     tracing::error!("bad query: {e:?}");
    //     e
    // })
    ?;
    Ok(())
}

/// `GET /subscriptions/confirm`
///
/// Given a token in `params`, get the user id associated with it, then change
/// the user's `status` to confirmed.
///
/// Failure to parse `params` will automatically return 400.
#[tracing::instrument(name = "Confirming new subscriber", skip(params, pool))]
pub async fn confirm(
    params: Query<Parameters>,
    pool: web::Data<PgPool>,
    // ) -> HttpResponse {
) -> Result<HttpResponse, ConfirmError> {
    // extra: basic string validation: ensure token is 25 chars long, alphanumeric
    // (no spaces). entropy could also be checked (but this is probably
    // overkill)
    if params.subscription_token.len() != 25 || params.subscription_token.contains(' ') {
        return Ok(HttpResponse::InternalServerError().finish());
    }

    let id = get_subscriber_id_from_token(&pool, &params.subscription_token)
        .await
        .context("Failed to get subscriber id from token")?
        .ok_or(ConfirmError::ValidationError)?;

    // extra: prevent user from being confirmed twice (this is only a formality,
    // because `confirm_subscriber` is actually idempotent)
    if sqlx::query!(
        "
    SELECT status FROM subscriptions
    WHERE id = $1
",
        id,
    )
    .fetch_optional(pool.as_ref())
    .await
    .unwrap()
    .map(|u| u.status)
        == Some("confirmed".to_owned())
    {
        return Ok(HttpResponse::InternalServerError().finish());
    };

    confirm_subscriber(&pool, id)
        .await
        .context("Failed to confirm subscriber")?;

    Ok(HttpResponse::Ok().finish())
}
