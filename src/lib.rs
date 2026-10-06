pub mod config;
pub mod controllers;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod views;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub session_cookie_secure: bool,
}
