// a miniature yagami: fail-fast env config, postgres with migrations on boot, and a sibling binary
// (`worker`) spawned from next to the running exe. no SIGTERM handler, same as yagami.
use std::{env::current_exe, process::Stdio};

use axum::{Json, Router, extract::State, http::HeaderValue, routing::get};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower_http::cors::CorsLayer;

#[derive(Debug)]
struct Config {
    bind: String,
    database_url: String,
    allowed_origin: String,
    greeting: String,
}

impl Config {
    // takes the lookup as a function so tests can feed it a map instead of mutating the process env.
    fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Config, String> {
        let require = |name: &str| get(name).ok_or_else(|| format!("missing env var {name}"));
        Ok(Config {
            bind: require("SANDBOX_BIND")?,
            database_url: require("DATABASE_URL")?,
            allowed_origin: require("SANDBOX_ALLOWED_ORIGIN")?,
            greeting: require("SANDBOX_GREETING")?,
        })
    }
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    greeting: String,
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let config = Config::from_lookup(|name| std::env::var(name).ok()).unwrap();

    let pool = PgPool::connect(&config.database_url).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/hit", get(hit))
        .layer(CorsLayer::new().allow_origin(config.allowed_origin.parse::<HeaderValue>().unwrap()))
        .with_state(AppState { pool, greeting: config.greeting });

    let listener = tokio::net::TcpListener::bind(&config.bind).await.unwrap();
    println!("listening on {}", config.bind);
    axum::serve(listener, app).await.unwrap();
}

async fn hit(State(state): State<AppState>) -> Json<Value> {
    let count: i64 = sqlx::query_scalar("UPDATE hits SET count = count + 1 WHERE id = 1 RETURNING count")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    Json(json!({ "count": count, "shout": shout(&state.greeting).await }))
}

// the same lookup yagami's spawn_runtime does: the worker must sit in the same directory as this exe.
async fn shout(text: &str) -> String {
    let mut child = tokio::process::Command::new(
        current_exe().unwrap().parent().unwrap().join(format!("worker{}", std::env::consts::EXE_SUFFIX)),
    )
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to spawn worker");

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(text.as_bytes()).await.unwrap();
    drop(stdin);

    let mut out = String::new();
    child.stdout.take().unwrap().read_to_string(&mut out).await.unwrap();
    child.wait().await.unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> =
            vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn config_reads_every_var() {
        let config = Config::from_lookup(lookup(&[
            ("SANDBOX_BIND", "127.0.0.1:8080"),
            ("DATABASE_URL", "postgres://x"),
            ("SANDBOX_ALLOWED_ORIGIN", "http://localhost:5173"),
            ("SANDBOX_GREETING", "hello"),
        ]))
        .unwrap();
        assert_eq!(config.greeting, "hello");
    }

    #[test]
    fn config_names_the_missing_var() {
        let err = Config::from_lookup(lookup(&[("SANDBOX_BIND", "127.0.0.1:8080")])).unwrap_err();
        assert_eq!(err, "missing env var DATABASE_URL");
    }
}
