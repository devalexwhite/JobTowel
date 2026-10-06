use serde::Deserialize;
use sqlx::{FromRow, SqlitePool};

pub const MAX_COMMENT_LENGTH: usize = 2000;

#[derive(Debug, FromRow)]
pub struct Comment {
    pub id: i64,
    pub body: String,
    pub created_at: String,
}

#[derive(Default, Deserialize)]
pub struct NewComment {
    #[serde(default)]
    pub body: String,
}

pub struct ValidatedComment {
    body: String,
}

impl NewComment {
    pub fn validate(&self) -> Result<ValidatedComment, &'static str> {
        let body = self.body.trim();
        if body.is_empty() {
            return Err("Write a comment before posting.");
        }
        if body.chars().count() > MAX_COMMENT_LENGTH {
            return Err("Keep your comment to 2,000 characters or fewer.");
        }
        if body.contains('\0') {
            return Err("Remove null characters from your comment.");
        }
        Ok(ValidatedComment {
            body: body.to_owned(),
        })
    }
}

impl Comment {
    pub async fn all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT id, body, created_at FROM comments ORDER BY id DESC")
            .fetch_all(pool)
            .await
    }

    pub async fn create(pool: &SqlitePool, comment: ValidatedComment) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "INSERT INTO comments (body) VALUES (?) RETURNING id, body, created_at",
        )
        .bind(comment.body)
        .fetch_one(pool)
        .await
    }
}
