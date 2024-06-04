// "We neglected one detail: there is no expiry mechanism for our idempotency
// keys. Try designing one as an exercise, using what we learned on background
// workers as a reference."

// this worker is solely responsible for periodically checking the `idempotency`
// table and dropping rows with `created_at` >24 h

use std::time::Duration;

use sqlx::PgPool;

use crate::configuration::Settings;
use crate::startup::get_connection_pool;

async fn expire_old_keys(pool: &PgPool) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        // https://stackoverflow.com/a/13828231
        // https://www.postgresql.org/docs/current/datatype-datetime.html
        r#"
        DELETE FROM idempotency
        WHERE now() - created_at > interval '24 hours'
"#,
    );
    query.execute(pool).await?;
    Ok(())
}

async fn expire_keys_loop(pool: &PgPool) -> Result<(), anyhow::Error> {
    loop {
        match expire_old_keys(pool).await {
            Err(_) => tokio::time::sleep(Duration::from_secs(60)).await,
            Ok(_) => tokio::time::sleep(Duration::from_secs(600)).await,
        }
    }
}

/// To be run as a separate worker, outside the main API
pub async fn init_expiry_worker(cfg: Settings) -> Result<(), anyhow::Error> {
    let pool = get_connection_pool(&cfg.database);
    expire_keys_loop(&pool).await
}
