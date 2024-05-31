use actix_web::body::to_bytes;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use sqlx::postgres::PgHasArrayType;
use sqlx::Executor;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::Transaction;
use uuid::Uuid;

use super::IdempotencyKey;

// both derive and sqlx are required
#[derive(sqlx::Type, Debug)]
#[sqlx(type_name = "header_pair")] // tell sqlx about the composite type
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

// tell sqlx about the array containing the composite type
impl PgHasArrayType for HeaderPairRecord {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_header_pair")
    }
}

// In-memory locks (e.g. tokio::sync::Mutex) would work if all incoming requests
// were being served by a single API instance. This is not our case: our API is
// replicated, therefore the two requests might end up being processed by two
// different instances. Our synchronization mechanism will have to live
// out-of-process - our database being the natural candidate.
/// Used to achieve concurrency on a database level
pub enum NextAction {
    // StartProcessing,
    StartProcessing(Transaction<'static, Postgres>),
    /// Wrapper for a redirect
    ReturnSavedResponse(HttpResponse),
}

/// Begin a transaction (which will be returned), and insert a partially filled
/// record (without a HTTP response). Should be invoked before undertaking
/// actions that affect users.
///
/// Because of the transaction, any number of requests can be made, but only one
/// will succeed.
pub async fn try_save_response(
    user_id: Uuid,
    idempotency_key: &IdempotencyKey,
    pool: &PgPool,
) -> Result<NextAction, anyhow::Error> {
    let mut transaction = pool.begin().await?;

    // https://www.postgresql.org/docs/current/transaction-iso.html#XACT-REPEATABLE-READ
    // https://www.cybertec-postgresql.com/en/transactions-in-postgresql-read-committed-vs-repeatable-read/#transaction-isolation-in-postgresql-visualized

    // // repeatable read: "the same SELECT query, if run twice in a row within the
    // // -same- transaction, will always return the same data" (that was returned
    // // at the start, i.e. "frozen")
    // transaction
    //     .execute(sqlx::query!(
    //         "SET TRANSACTION ISOLATION LEVEL repeatable read"
    //     ))
    //     .await?;

    let query = sqlx::query!(
        r#"
        INSERT INTO idempotency
            (user_id, idempotency_key, created_at)
        VALUES
            ($1, $2, now())
        ON CONFLICT DO NOTHING
    "#,
        user_id,
        idempotency_key.as_ref(),
    );

    let next = match
        // query.execute(pool)
        transaction.execute(query)
        .await?.rows_affected() > 0 {
        // insert successful -> new request -> caller can go ahead (and later save the complete
        // response)
        true => NextAction::StartProcessing(transaction),
        // request was already made -> check if saved response is complete -> if yes, pass it to
        // caller so it can return early, else abort (as another request must be ongoing)
        false => {
            let resp = get_saved_response(user_id, idempotency_key, pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("could not retrieve saved response"))?;
            NextAction::ReturnSavedResponse(resp)
        }
    };
    Ok(next)
}

/// "`Serialize`": `HttpResponse` -> SQL row
///
/// Update a partially filled record.
pub async fn save_response(
    user_id: Uuid,
    idempotency_key: &IdempotencyKey,
    http_response: HttpResponse, // will usually be a redirect
    // pool: &PgPool,
    mut transaction: Transaction<'static, Postgres>,
) -> Result<HttpResponse, anyhow::Error> {
    // StatusCode -> u16 -> i16
    let status_code = http_response.status().as_u16() as i16;

    let mut raw_headers = Vec::with_capacity(http_response.headers().len());
    for (name, value) in http_response.headers() {
        let name = name.as_str().to_owned();
        let value = value.as_bytes().to_vec();
        let pair = HeaderPairRecord { name, value };
        raw_headers.push(pair);
    }

    // note the generic return type (B); html response can be html, json, binary,
    // etc. if unspecified (in most cases), `B` = `BoxBody`.
    //
    // `BoxBody` uses the strategy pattern to determine how to handle streaming, and
    // one strategy requires the `MessageBody` trait. `MessageBody` takes care
    // of HTTP streaming if the response is large (and must be chunked)

    // // .body returns &BoxBody -- neither clone/to_owned will turn it into BoxBody
    // let body = http_response.body();

    let (head, body) = http_response.into_parts();

    let raw_body = to_bytes(body).await.map_err(
        // because `MessageBody::Error` does not implement `Send`/`Sync`, it cannot be coerced into
        // `anyhow::Error`
        // error_500
        |e| anyhow::anyhow!("{e}"),
    )?; // `to_bytes` requires `BoxBody`

    // query_unchecked required for custom type
    let query = sqlx::query_unchecked!(
        r#"
        -- INSERT INTO idempotency
        --     (
        --     user_id,
        --     idempotency_key,
        --     created_at,
        --     response_status_code,
        --     response_headers,
        --     response_body
        --     )
        -- VALUES
        --     (
        --     $1,
        --     $2,
        --     now(), -- https://www.postgresql.org/docs/current/functions-datetime.html#FUNCTIONS-DATETIME-CURRENT
        --     $3,
        --     $4,
        --     $5
        --     )
        UPDATE idempotency
        SET
            response_status_code = $3,
            response_headers = $4,
            response_body = $5
        WHERE
            user_id = $1 AND
            idempotency_key = $2
        "#,
        user_id,
        idempotency_key.as_ref(),
        status_code,
        raw_headers,
        raw_body.as_ref(), // Bytes -> literal bytes (&[u8])
    );
    // query.execute(pool).await?;
    transaction.execute(query).await?;
    transaction.commit().await?; // this is the last action!

    // reconstruct the original response and return it (we can no longer use
    // `http_response` due to the `.into_parts` call, and `HttpResponse` cannot be
    // `clone`d)
    let http_response = head.set_body(raw_body).map_into_boxed_body();
    Ok(http_response)
}

/// "`Deserialize`": SQL row -> `HttpResponse`
///
/// Retrieves a complete record (i.e. with no nulls, and a fully formed HTTP
/// response). `save_response` is responsible for ensuring completeness of the
/// record.
pub async fn get_saved_response(
    user_id: Uuid,
    idempotency_key: &IdempotencyKey,
    pool: &PgPool,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    let saved_response = sqlx::query!(
        r#"
        SELECT 
            -- response_status_code,
            -- -- response_headers, -- composite type must be "deserialised"
            -- response_headers as "response_headers: Vec<HeaderPairRecord>", 
            -- response_body

            -- automatically unwrap Some(x); but what happens to nulls?
            -- https://docs.rs/sqlx/latest/sqlx/macro.query.html#type-overrides-output-columns
            response_status_code as "response_status_code!",
            response_headers as "response_headers!: Vec<HeaderPairRecord>", 
            response_body as "response_body!"
        FROM idempotency
        WHERE 
            user_id = $1 AND
            idempotency_key = $2
        "#,
        user_id,
        idempotency_key.as_ref()
    )
    .fetch_optional(pool)
    .await?;
    let resp = match saved_response {
        None => None,
        Some(saved) => {
            // i16 -> u16 -> StatusCode
            let status_code = StatusCode::from_u16(saved.response_status_code.try_into()?)?;
            let mut resp = HttpResponse::build(status_code);
            // struct unpacking
            for HeaderPairRecord { name, value } in saved.response_headers {
                resp.append_header((name, value));
            }
            let resp = resp.body(saved.response_body);
            Some(resp)
        }
    };
    Ok(resp)
}
