use crate::models::dates;
use chrono::{Duration, NaiveDate, SecondsFormat, Utc};
use sqlx::{FromRow, SqlitePool};

pub struct Summary {
    pub application_count: i64,
    pub latest_movement: Option<ApplicationEvent>,
    pub upcoming_event: Option<ApplicationEvent>,
    pub milestone_days: Vec<MilestoneDay>,
}

pub struct MilestoneDay {
    pub date: NaiveDate,
    pub events: Vec<ApplicationEvent>,
}

#[derive(FromRow)]
pub struct ApplicationEvent {
    pub role_id: i64,
    pub role_title: String,
    pub company: String,
    pub event_title: String,
    pub occurs_at: String,
    pub days_until: i64,
}

impl Summary {
    pub async fn load(pool: &SqlitePool, user_id: i64) -> Result<Self, sqlx::Error> {
        let mut tx = pool.begin().await?;
        // Use one instant and snapshot for all homepage cards.
        let instant = Utc::now();
        let now = instant.to_rfc3339_opts(SecondsFormat::Millis, true);
        let timezone: Option<String> =
            sqlx::query_scalar("SELECT timezone FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?;
        let timezone = timezone.as_deref().unwrap_or("UTC");
        let application_count =
            sqlx::query_scalar("SELECT COUNT(*) FROM roles WHERE user_id = ? AND active = 1")
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?;
        let mut latest_movement: Option<ApplicationEvent> = sqlx::query_as("SELECT roles.id AS role_id, roles.role_title, roles.company, events.title AS event_title, events.occurs_at, 0 AS days_until FROM events JOIN roles ON roles.id = events.role_id WHERE roles.user_id = ? AND events.occurs_at <= ? ORDER BY events.occurs_at DESC, events.id DESC LIMIT 1")
            .bind(user_id).bind(&now).fetch_optional(&mut *tx).await?;
        let mut upcoming_event: Option<ApplicationEvent> = sqlx::query_as("SELECT roles.id AS role_id, roles.role_title, roles.company, events.title AS event_title, events.occurs_at, 0 AS days_until FROM events JOIN roles ON roles.id = events.role_id WHERE roles.user_id = ? AND events.occurs_at > ? ORDER BY events.occurs_at ASC, events.id ASC LIMIT 1")
            .bind(user_id).bind(&now).fetch_optional(&mut *tx).await?;
        for event in [&mut latest_movement, &mut upcoming_event]
            .into_iter()
            .flatten()
        {
            event.days_until = dates::days_until(&event.occurs_at, instant, timezone).unwrap_or(0);
        }
        let today = instant.with_timezone(&dates::zone(timezone)).date_naive();
        let mut milestone_days: Vec<_> = (0..14)
            .map(|days| MilestoneDay {
                date: today - Duration::days(days),
                events: Vec::new(),
            })
            .collect();
        // Pad the UTC query window to include every local day across timezone
        // offsets and clock changes; group and trim by local calendar date below.
        let start = (instant - Duration::days(15)).to_rfc3339_opts(SecondsFormat::Millis, true);
        let end = (instant + Duration::days(2)).to_rfc3339_opts(SecondsFormat::Millis, true);
        let events: Vec<ApplicationEvent> = sqlx::query_as("SELECT roles.id AS role_id, roles.role_title, roles.company, events.title AS event_title, events.occurs_at, 0 AS days_until FROM events JOIN roles ON roles.id = events.role_id WHERE roles.user_id = ? AND events.occurs_at >= ? AND events.occurs_at < ? ORDER BY events.occurs_at DESC, events.id DESC")
            .bind(user_id).bind(start).bind(end).fetch_all(&mut *tx).await?;
        for event in events {
            if let Some(local) = dates::local(&event.occurs_at, timezone)
                && let Some(day) = milestone_days
                    .iter_mut()
                    .find(|day| day.date == local.date_naive())
            {
                day.events.push(event);
            }
        }
        tx.commit().await?;
        Ok(Self {
            application_count,
            latest_movement,
            upcoming_event,
            milestone_days,
        })
    }
}
