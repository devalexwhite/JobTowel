use axum::http::Uri;
use serde::Deserialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, FromRow)]
pub struct Role {
    pub id: i64,
    pub role_title: String,
    pub posting_url: String,
    pub company: String,
    pub role_description: String,
    pub user_id: i64,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, FromRow)]
pub struct AppliedRole {
    #[sqlx(flatten)]
    pub role: Role,
    pub last_event_title: Option<String>,
    pub last_event_at: Option<String>,
    pub last_event_days_ago: Option<i64>,
}
impl AppliedRole {
    pub async fn list(pool: &SqlitePool, user_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as(
            "SELECT roles.*, events.title AS last_event_title, events.occurs_at AS last_event_at, CAST(julianday(date('now')) - julianday(date(events.occurs_at)) AS INTEGER) AS last_event_days_ago FROM roles LEFT JOIN events ON events.id = (SELECT id FROM events WHERE role_id = roles.id ORDER BY occurs_at DESC, id DESC LIMIT 1) WHERE roles.user_id = ? ORDER BY roles.created_at DESC, roles.id DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }
}
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Input {
    pub role_title: String,
    pub posting_url: String,
    pub company: String,
    pub role_description: String,
    pub csrf: String,
}
pub struct ValidatedInput {
    role_title: String,
    posting_url: String,
    company: String,
    role_description: String,
}
impl Input {
    pub fn validate(&self) -> Result<ValidatedInput, &'static str> {
        for (value, error) in [
            (
                &self.role_title,
                "Enter a role title between 1 and 200 characters.",
            ),
            (
                &self.company,
                "Enter a company between 1 and 200 characters.",
            ),
        ] {
            if value.trim().is_empty()
                || value.trim().chars().count() > 200
                || value.chars().any(char::is_control)
            {
                return Err(error);
            }
        }
        let url = self.posting_url.trim();
        if !url.is_empty()
            && (url.len() > 2048
                || url.chars().any(|c| c.is_whitespace() || c.is_control())
                || !url.parse::<Uri>().is_ok_and(|uri| {
                    matches!(uri.scheme_str(), Some("http" | "https"))
                        && uri.host().is_some_and(|h| !h.is_empty())
                }))
        {
            return Err(
                "Enter a valid posting URL starting with http:// or https://, or leave it blank.",
            );
        }
        if self.role_description.chars().count() > 10000 {
            return Err("Keep the role description to 10,000 characters or fewer.");
        }
        Ok(ValidatedInput {
            role_title: self.role_title.trim().into(),
            posting_url: url.into(),
            company: self.company.trim().into(),
            role_description: self.role_description.trim().into(),
        })
    }
}
impl From<&Role> for Input {
    fn from(role: &Role) -> Self {
        Self {
            role_title: role.role_title.clone(),
            posting_url: role.posting_url.clone(),
            company: role.company.clone(),
            role_description: role.role_description.clone(),
            csrf: String::new(),
        }
    }
}
impl Role {
    pub async fn list(pool: &SqlitePool, user_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM roles WHERE user_id = ? ORDER BY created_at DESC, id DESC")
            .bind(user_id)
            .fetch_all(pool)
            .await
    }
    pub async fn find(
        pool: &SqlitePool,
        user_id: i64,
        id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM roles WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(id)
            .fetch_optional(pool)
            .await
    }
    pub async fn create(
        pool: &SqlitePool,
        user_id: i64,
        input: ValidatedInput,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as("INSERT INTO roles (role_title, posting_url, company, role_description, user_id) VALUES (?, ?, ?, ?, ?) RETURNING *").bind(input.role_title).bind(input.posting_url).bind(input.company).bind(input.role_description).bind(user_id).fetch_one(pool).await
    }
    pub async fn update(
        pool: &SqlitePool,
        user_id: i64,
        id: i64,
        input: ValidatedInput,
    ) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("UPDATE roles SET role_title = ?, posting_url = ?, company = ?, role_description = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE user_id = ? AND id = ?").bind(input.role_title).bind(input.posting_url).bind(input.company).bind(input.role_description).bind(user_id).bind(id).execute(pool).await?.rows_affected() == 1)
    }
    pub async fn delete(pool: &SqlitePool, user_id: i64, id: i64) -> Result<bool, sqlx::Error> {
        Ok(
            sqlx::query("DELETE FROM roles WHERE user_id = ? AND id = ?")
                .bind(user_id)
                .bind(id)
                .execute(pool)
                .await?
                .rows_affected()
                == 1,
        )
    }
}
