use crate::error::AppError;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use rand::{RngCore, rngs::OsRng};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use subtle::ConstantTimeEq;

// Public dummy hash makes unknown-user login perform one normal verification.
pub const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$h1c/ysIXdgi31mq9LitYIA$Ti7XzAhFqArD3UsiAfG/hPKXJ0RKHVvPrcita/FeoV8";

#[derive(Clone, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub name: String,
    pub email: String,
    pub password_hash: String,
}
#[derive(Default, Deserialize)]
pub struct Input {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub current_password: String,
    #[serde(default)]
    pub csrf: String,
}
pub struct Profile {
    name: String,
    email: String,
}
impl Input {
    pub fn profile(&self) -> Result<Profile, &'static str> {
        let name = self.name.trim();
        let email = self.email.trim();
        if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
            return Err("Enter a name between 1 and 100 characters.");
        }
        let valid_email = email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !domain.contains('@')
        });
        if email.len() > 254
            || !valid_email
            || email.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err("Enter a valid email address.");
        }
        Ok(Profile {
            name: name.into(),
            email: email.to_ascii_lowercase(),
        })
    }
    pub fn signup(&self) -> Result<Profile, &'static str> {
        let username = self.username.trim();
        if !(3..=32).contains(&username.len())
            || !username
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("Use 3–32 letters, numbers, underscores or hyphens for your username.");
        }
        validate_password(&self.password)?;
        self.profile()
    }
}
pub fn validate_password(password: &str) -> Result<(), &'static str> {
    if !(6..=128).contains(&password.chars().count()) {
        Err("Use a password with between 6 and 128 characters.")
    } else {
        Ok(())
    }
}
pub async fn hash_password(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|hash| hash.to_string())
            .map_err(|_| AppError::Authentication)
    })
    .await
    .map_err(|_| AppError::Authentication)?
}
pub async fn verify(password: String, hash: String) -> Result<bool, AppError> {
    if password.len() > 512 {
        return Ok(false);
    }
    tokio::task::spawn_blocking(move || {
        let hash = PasswordHash::new(&hash).map_err(|_| AppError::Authentication)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok())
    })
    .await
    .map_err(|_| AppError::Authentication)?
}
impl User {
    pub async fn by_username(
        pool: &SqlitePool,
        username: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM users WHERE username = ?")
            .bind(username.trim())
            .fetch_optional(pool)
            .await
    }
    pub async fn by_id(pool: &SqlitePool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }
    pub async fn create(
        pool: &SqlitePool,
        username: &str,
        profile: Profile,
        hash: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as("INSERT INTO users (username, name, email, password_hash) VALUES (?, ?, ?, ?) RETURNING *")
            .bind(username.trim()).bind(profile.name).bind(profile.email).bind(hash).fetch_one(pool).await
    }
    pub async fn update(
        &self,
        pool: &SqlitePool,
        profile: Profile,
        hash: Option<String>,
    ) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query("UPDATE users SET name = ?, email = ?, password_hash = ? WHERE id = ?")
            .bind(profile.name)
            .bind(profile.email)
            .bind(hash.as_ref().unwrap_or(&self.password_hash))
            .bind(self.id)
            .execute(&mut *tx)
            .await?;
        if hash.is_some() {
            sqlx::query("DELETE FROM sessions WHERE user_id = ?")
                .bind(self.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }
}
#[derive(FromRow)]
pub struct Session {
    pub user_id: Option<i64>,
    pub csrf: String,
}
pub fn token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
impl Session {
    pub async fn find(pool: &SqlitePool, token: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as(
            "SELECT user_id, csrf FROM sessions WHERE token_hash = ? AND expires_at > unixepoch()",
        )
        .bind(digest(token))
        .fetch_optional(pool)
        .await
    }
    pub async fn create(
        pool: &SqlitePool,
        user_id: Option<i64>,
    ) -> Result<(String, Self), sqlx::Error> {
        let token = token();
        let csrf = self::token();
        sqlx::query("DELETE FROM sessions WHERE expires_at <= unixepoch()")
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO sessions (token_hash, user_id, csrf, expires_at) VALUES (?, ?, ?, unixepoch() + ?)")
            .bind(digest(&token)).bind(user_id).bind(&csrf).bind(if user_id.is_some() { 604800 } else { 3600 }).execute(pool).await?;
        Ok((token, Self { user_id, csrf }))
    }
    pub async fn delete(pool: &SqlitePool, token: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(digest(token))
            .execute(pool)
            .await?;
        Ok(())
    }
    pub fn accepts(&self, csrf: &str) -> bool {
        bool::from(self.csrf.as_bytes().ct_eq(csrf.as_bytes()))
    }
}
