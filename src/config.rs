use std::{env, net::SocketAddr, path::PathBuf};

pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub asset_dir: PathBuf,
    pub session_cookie_secure: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self {
            session_cookie_secure: env::var("SESSION_COOKIE_SECURE")
                .unwrap_or_else(|_| "false".into())
                .parse()?,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://jobtowel.db".into()),
            bind_addr: env::var("BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:3000".into())
                .parse()?,
            asset_dir: env::var("ASSET_DIR")
                .unwrap_or_else(|_| "static/assets".into())
                .into(),
        })
    }
}
