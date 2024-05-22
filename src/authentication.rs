// any API can expect to encounter 3 types of clients, each with different modes
// of authentication:
//
// 1. another API (machine) -- request signing, mutual TLS, OAuth2, JWT
// 2. another API (human) -- OAuth2 (scoped)
// 3. browser (human) -- session-based authentication (login form), identity
//    federation
//
// #3 will be our main target

use anyhow::Context;
use argon2::Argon2;
use argon2::PasswordHash;
use argon2::PasswordVerifier;
use secrecy::ExposeSecret;
use secrecy::Secret;
use sqlx::PgPool;
use tokio::task::JoinHandle;
use uuid::Uuid;

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

pub async fn get_stored_credentials(
    username: String,
    pool: &PgPool,
    // returning `Record` is not allowed, unfortunately...
) -> Result<(Uuid, Secret<String>), AuthError> {
    let row = sqlx::query!(
        "
        SELECT user_id, password_hash -- , salt
        FROM users
        WHERE username = $1
        -- AND password_hash = $2
    ",
        username,
        // format!("{password_hash:x}"), // GenericArray -> hexadecimal
    )
    .fetch_optional(pool)
    .await
    .context("Failed to query db")
    .map_err(AuthError::UnexpectedError)?
    // note: the book just uses `map` to unpack the fields from within the `Some`, thus returning a
    // `Result<Option<(...)>>`. to streamline things, i use `map_err` (again) to convert `Option` to
    // `Result`, and lift the fields from `Some`
    .context("No user with the supplied username was found in users table")
    .map_err(AuthError::InvalidCredentials)?;
    Ok((row.user_id, Secret::new(row.password_hash)))
}

/// Note that verification is a CPU-bound operation that is fairly slow (by
/// design)
// up to 0.5 s (!)
// TEST_LOG=true cargo test confirmed | grep VERIF | bunyan
fn verify_password(
    supplied_password: Secret<String>,
    stored_password: Secret<String>,
) -> Result<(), AuthError> {
    let stored_password = &PasswordHash::new(stored_password.expose_secret())
        .context("Failed to read stored PHC string")
        .map_err(AuthError::UnexpectedError)?;
    Argon2::default()
        .verify_password(
            supplied_password.expose_secret().as_bytes(),
            stored_password,
        )
        .context("Invalid password")
        .map_err(AuthError::InvalidCredentials)?;
    Ok(())
}

// on salting: "For each user, we generate a unique random string (salt), which
// is prepended to the user password before generating the hash." The salt does
// not need to be hashed, just random.
//
// this means we need to validate in 2 steps: first query `users` table to get
// the user's salt, then use the salt to calculate the hashed password
//
// however, notice now that, in comparison to our initial sha3 implementation,
// hashing now involves several parameters, which are defined in this function.
// if we were to change any of these parameters, and lose information on the
// params used to generate the existing hashes, authentication would break
// completely. thus, all such variable params must be stored in the db
// in PHC format, which captures all necessary information in a
// single string
//
/// Validate supplied credentials (username/password) by checking against the
/// `users` table in db, returning the user's `Uuid` on success.
#[tracing::instrument(name = "Validating credentials", skip(creds, pool))]
pub async fn validate_credentials(
    creds: Credentials,
    pool: &PgPool,
    // ) -> Result<Uuid, PublishError> {
) -> Result<Uuid, AuthError> {
    // let (user_id, stored_password) = get_stored_credentials(creds.username,
    // pool).await?;

    let (user_id, stored_password) = match get_stored_credentials(creds.username, pool).await {
        Ok((i, p)) => (i, p),
        // Notice that returning early here skips the (slow) hash verification, leading to a 10x
        // 'speedup'. This may be exploited for a timing attack, allowing attackers to
        // perform user enumeration and determine which usernames are valid (and which
        // aren't). To avoid this, use a fallback hash (which must be a valid PHC with the same
        // params; otherwise verification will also be quick) to ensure constant computation time
        // regardless of user validity.
        Err(_) => (
            Uuid::new_v4(), // dummy, will not be returned
            Secret::new(
                // these argon2 params correspond with those declared in `TestUser.store`
                // # ${algo}${algo version}${params (,-separated)}${hash}${salt}
                // whitespace is ignored
                "$argon2id$v=19$m=19456,t=2,p=1\
                $gZiV/M1gPc22ElAH/Jh1Hw\
                $CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
                    .to_string(),
            ),
        ),
    };

    // use sha3::Digest;
    // use sha3::Sha3_256;
    // let hashed_pw = Sha3_256::digest(creds.password.expose_secret().as_bytes());

    // let hashed_pw = Argon2::new(
    //     // OWASP recommendation
    //     argon2::Algorithm::Argon2id,
    //     argon2::Version::V0x13,
    //     argon2::Params::new(15000, 2, 1, None).unwrap(),
    // )
    // // requires `PasswordHasher` trait.
    // // note: argon2 0.5.3 no longer allows the `salt` arg to be a `&str`, hence
    // // the lengthy type conversion (String -> bytes -> b64 -> Result<Salt>)
    // .hash_password(
    //     creds.password.expose_secret().as_bytes(),
    //     // &stored.salt
    //     Salt::from_b64(&general_purpose::STANDARD.encode(stored.salt.
    // as_bytes())).unwrap(), )
    // .unwrap();

    // "[a] future progresses from the previous .await to the next one and then
    // yields control back to the executor."
    //
    // "async runtimes make progress concurrently on multiple I/O-bound tasks by
    // continuously parking and resuming each of them." generally, any task that
    // takes more than 1 ms can be said to be CPU-bound, and should be handed
    // off to a separate threadpool (that does -not- yield)

    /// Wrapper for `spawn_blocking` with `tracing`
    pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let span = tracing::Span::current();

        tokio::task::spawn_blocking(move || {
            // tracing::info_span!("Verifying password hash").in_scope(|| {
            span.in_scope(
                // 1. `verify_password` strictly requires both args to be refs (`to_owned` won't
                //    work)
                // 2. `move`ing refs into a thread is forbidden by the borrow checker; a thread
                //    spawned by `spawn_blocking` is assumed to last for the duration of the entire
                //    program
                // 3. we want to be able catch `Err` from `PasswordHash::new`; this is not trivial
                //    from within a thread
                //
                // instead, only owned data should be moved into the thread

                // Argon2::default().verify_password(
                //     creds.password.expose_secret().as_bytes(),
                //     &PasswordHash::new(stored_password.expose_secret())
                //         .context("Failed to read stored PHC string")
                //         .map_err(PublishError::UnexpectedError)
                //         .unwrap(),
                // )
                f,
            )
        })
    }

    // notice that there are 2 closures: the function (`verify_password`) is first
    // placed in a tracing span, and this span is then placed in a blocking
    // thread
    spawn_blocking_with_tracing(move || verify_password(creds.password, stored_password))
        .await
        .context("Failed to spawn blocking thread")
        .map_err(AuthError::UnexpectedError)?
        .context("Invalid password")
        .map_err(AuthError::InvalidCredentials)?;

    Ok(user_id)
    // "invalid username" error is already handled in `get_stored_credentials`
    // user_id.ok_or_else(|| PublishError::AuthError(anyhow::anyhow!("Invalid
    // username")))
}
