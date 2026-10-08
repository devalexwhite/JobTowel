use crate::models::{auth::User, dates};
use axum::http::Uri;
use chrono::Utc;
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
    pub active: bool,
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
    pub first_event_at: Option<String>,
    pub active_days: Option<i64>,
}
impl AppliedRole {
    pub async fn list(pool: &SqlitePool, user_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        let timezone = User::by_id(pool, user_id)
            .await?
            .map(|user| user.timezone().to_owned())
            .unwrap_or_else(|| "UTC".into());
        let mut roles: Vec<Self> = sqlx::query_as(
            "SELECT roles.*, events.title AS last_event_title, events.occurs_at AS last_event_at, NULL AS last_event_days_ago, (SELECT MIN(occurs_at) FROM events WHERE role_id = roles.id) AS first_event_at, NULL AS active_days FROM roles LEFT JOIN events ON events.id = (SELECT id FROM events WHERE role_id = roles.id ORDER BY occurs_at DESC, id DESC LIMIT 1) WHERE roles.user_id = ? ORDER BY roles.created_at DESC, roles.id DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        let now = Utc::now();
        for role in &mut roles {
            role.active_days = role.first_event_at.as_deref().and_then(|date| {
                dates::days_until(date, now, &timezone).map(|days| (-days).max(0))
            });
            role.last_event_days_ago = role
                .last_event_at
                .as_deref()
                .and_then(|date| dates::days_until(date, now, &timezone))
                .map(|days| -days);
        }
        Ok(roles)
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
    pub active: Option<String>,
    pub status_present: String,
}
pub struct ValidatedInput {
    role_title: String,
    posting_url: String,
    company: String,
    role_description: String,
    active: bool,
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
        if self
            .active
            .as_deref()
            .is_some_and(|value| !["true", "false"].contains(&value))
        {
            return Err("Choose an active or inactive application status.");
        }
        Ok(ValidatedInput {
            role_title: self.role_title.trim().into(),
            posting_url: url.into(),
            company: self.company.trim().into(),
            role_description: self.role_description.trim().into(),
            active: if self.status_present == "true" {
                self.active.as_deref() == Some("true")
            } else {
                self.active.as_deref() != Some("false")
            },
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
            active: Some(role.active.to_string()),
            status_present: "true".into(),
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
        sqlx::query_as("INSERT INTO roles (role_title, posting_url, company, role_description, user_id, active) VALUES (?, ?, ?, ?, ?, ?) RETURNING *").bind(input.role_title).bind(input.posting_url).bind(input.company).bind(input.role_description).bind(user_id).bind(input.active).fetch_one(pool).await
    }
    pub async fn update(
        pool: &SqlitePool,
        user_id: i64,
        id: i64,
        input: ValidatedInput,
    ) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("UPDATE roles SET role_title = ?, posting_url = ?, company = ?, role_description = ?, active = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE user_id = ? AND id = ?").bind(input.role_title).bind(input.posting_url).bind(input.company).bind(input.role_description).bind(input.active).bind(user_id).bind(id).execute(pool).await?.rows_affected() == 1)
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

#[derive(Default, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum StatusFilter {
    #[default]
    Active,
    Inactive,
    All,
}
impl StatusFilter {
    pub fn value(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::All => "all",
        }
    }
    pub fn matches(self, role: &Role) -> bool {
        match self {
            Self::Active => role.active,
            Self::Inactive => !role.active,
            Self::All => true,
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ListInput {
    pub status: StatusFilter,
}
