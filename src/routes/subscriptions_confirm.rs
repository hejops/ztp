use actix_web::web::Query;
use actix_web::web::{self};
use actix_web::HttpResponse;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

/// Fails if `token` not found in `subscription_tokens` table. The `id` returned
/// may be empty, so this should be checked by the caller.
#[tracing::instrument(name = "Getting id of new subscriber", skip(pool, token))]
async fn get_subscriber_id_from_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let id = sqlx::query!(
        "
    SELECT subscriber_id FROM subscription_tokens
    WHERE subscription_token = $1
",
        token,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("bad query: {e:?}");
        e
    })?
    .map(|u| u.subscriber_id);
    Ok(id)
}

#[tracing::instrument(name = "Changing status of new subscriber", skip(pool, id))]
async fn confirm_subscriber(
    pool: &PgPool,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "
    UPDATE subscriptions SET status = 'confirmed' WHERE id = $1
",
        id,
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("bad query: {e:?}");
        e
    })?;
    Ok(())
}

/// Given a token in `params`, get the user id associated with it, then change
/// the user's `status` to confirmed.
///
/// Failure to parse `params` will automatically return 400.
#[tracing::instrument(name = "Confirming new subscriber", skip(params, pool))]
pub async fn confirm(
    params: Query<Parameters>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    let id = match get_subscriber_id_from_token(&pool, &params.subscription_token).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let id = match id {
        Some(id) => id,
        None => return HttpResponse::InternalServerError().finish(),
    };

    match confirm_subscriber(&pool, id).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
