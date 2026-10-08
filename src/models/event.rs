use crate::models::{
    dates,
    person::{self, Person},
};
use chrono::Utc;
use serde::{Deserialize, Deserializer};
use sqlx::{FromRow, SqlitePool};
use std::collections::{BTreeMap, HashMap};

pub const TITLE_OPTIONS: [&str; 10] = [
    "Removed from consideration",
    "Interview scheduled",
    "Interview completed",
    "Take home assigned",
    "Take home completed",
    "Offer extended",
    "Offer rejected",
    "Offer accepted",
    "Recruiter screen scheduled",
    "Recruiter screen completed",
];

#[derive(Debug, FromRow)]
pub struct Event {
    pub id: i64,
    pub role_id: i64,
    pub title: String,
    pub occurs_at: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Default)]
pub struct Input {
    pub title: String,
    pub title_choice: String,
    pub occurs_at: String,
    pub utc_offset: String,
    pub offset_for: String,
    pub timezone: String,
    pub notes: String,
    pub people: Vec<person::Input>,
    pub available_people: Vec<Person>,
    pub people_action: String,
    pub csrf: String,
    pub removal_status: String,
    pub status_prompt: bool,
}
// Indexed flat fields support ordinary URL-encoded browser forms and HTMX alike.
impl<'de> Deserialize<'de> for Input {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut fields = HashMap::<String, String>::deserialize(deserializer)?;
        let mut input = Self {
            title: fields.remove("title").unwrap_or_default(),
            title_choice: fields.remove("title_choice").unwrap_or_default(),
            occurs_at: fields.remove("occurs_at").unwrap_or_default(),
            utc_offset: fields.remove("utc_offset").unwrap_or_default(),
            offset_for: fields.remove("offset_for").unwrap_or_default(),
            timezone: fields.remove("timezone").unwrap_or_default(),
            notes: fields.remove("notes").unwrap_or_default(),
            removal_status: fields.remove("removal_status").unwrap_or_default(),
            csrf: fields.remove("csrf").unwrap_or_default(),
            people_action: fields.remove("people_action").unwrap_or_default(),
            ..Self::default()
        };
        let mut people = BTreeMap::<usize, person::Input>::new();
        for (key, value) in fields {
            let Some((index, field)) = key
                .strip_prefix("person_")
                .and_then(|key| key.split_once('_'))
            else {
                continue;
            };
            let index = index.parse::<usize>().map_err(serde::de::Error::custom)?;
            let person = people.entry(index).or_default();
            match field {
                "id" => person.id = value,
                "first_name" => person.first_name = value,
                "last_name" => person.last_name = value,
                "job_title" => person.job_title = value,
                "email" => person.email = value,
                "phone" => person.phone = value,
                "notes" => person.notes = value,
                _ => {}
            }
        }
        input.people = people.into_values().collect();
        Ok(input)
    }
}
pub struct ValidatedInput {
    title: String,
    occurs_at: String,
    notes: String,
    people: Vec<person::ValidatedInput>,
    deactivate: bool,
}
impl ValidatedInput {
    pub fn is_removal(&self) -> bool {
        self.title == "Removed from consideration"
    }
}
impl Input {
    pub fn for_new(timezone: &str) -> Self {
        let now = Utc::now().with_timezone(&dates::zone(timezone));
        Self {
            occurs_at: now.format("%Y-%m-%dT%H:%M").to_string(),
            utc_offset: now.format("%:z").to_string(),
            ..Self::default()
        }
    }

    pub fn validate(&self, timezone: &str) -> Result<ValidatedInput, &'static str> {
        let title = match self.title_choice.as_str() {
            "" | "custom" => self.title.trim(),
            choice if TITLE_OPTIONS.contains(&choice) => choice,
            _ => return Err("Choose an event title from the list, or select Custom."),
        };
        if title.is_empty() || title.chars().count() > 200 || title.chars().any(char::is_control) {
            return Err("Enter an event title between 1 and 200 characters.");
        }
        if !self.timezone.is_empty() && self.timezone != timezone {
            return Err(
                "Your account date settings changed. Check your timezone in account settings and the date and time below, then submit again.",
            );
        }
        let offset = if self.offset_for.is_empty() || self.offset_for == self.occurs_at {
            self.utc_offset.as_str()
        } else {
            ""
        };
        let occurs_at = dates::to_utc(&self.occurs_at, timezone, offset)?;
        if self.notes.chars().count() > 10000 {
            return Err("Keep notes to 10,000 characters or fewer.");
        }
        if self.people.len() > 20 {
            return Err("Add up to 20 people per event.");
        }
        let mut people = Vec::new();
        let mut selected = std::collections::HashSet::new();
        for person in &self.people {
            let person = person.validate()?;
            if let person::ValidatedInput::Existing(id) = &person
                && !selected.insert(*id)
            {
                return Err("Each person can only be added once per event.");
            }
            people.push(person);
        }
        Ok(ValidatedInput {
            title: title.into(),
            occurs_at,
            notes: self.notes.trim().into(),
            people,
            deactivate: title == "Removed from consideration" && self.removal_status == "inactive",
        })
    }
    pub fn from_event(event: &Event, people: &[Person], timezone: &str) -> Self {
        Self {
            title: event.title.clone(),
            title_choice: if TITLE_OPTIONS.contains(&event.title.as_str()) {
                event.title.clone()
            } else {
                "custom".into()
            },
            occurs_at: dates::local(&event.occurs_at, timezone)
                .map(|date| date.format("%Y-%m-%dT%H:%M").to_string())
                .unwrap_or_default(),
            utc_offset: dates::local(&event.occurs_at, timezone)
                .map(|date| date.format("%:z").to_string())
                .unwrap_or_default(),
            notes: event.notes.clone(),
            people: people.iter().map(person::Input::from_person).collect(),
            people_action: String::new(),
            csrf: String::new(),
            ..Self::default()
        }
    }
}
impl Event {
    pub async fn list(pool: &SqlitePool, user: i64, role: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as("SELECT events.* FROM events JOIN roles ON roles.id = events.role_id WHERE roles.user_id = ? AND role_id = ? ORDER BY occurs_at DESC, events.id DESC").bind(user).bind(role).fetch_all(pool).await
    }
    pub async fn find(
        pool: &SqlitePool,
        user: i64,
        role: i64,
        id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT events.* FROM events JOIN roles ON roles.id = events.role_id WHERE roles.user_id = ? AND role_id = ? AND events.id = ?").bind(user).bind(role).bind(id).fetch_optional(pool).await
    }
    pub async fn people(&self, pool: &SqlitePool) -> Result<Vec<Person>, sqlx::Error> {
        sqlx::query_as(
            "SELECT people.* FROM people JOIN event_people ON event_people.person_id = people.id WHERE event_people.event_id = ? ORDER BY people.id",
        )
        .bind(self.id)
        .fetch_all(pool)
        .await
    }
    pub async fn save(
        pool: &SqlitePool,
        user: i64,
        role: i64,
        id: Option<i64>,
        input: ValidatedInput,
    ) -> Result<bool, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let deactivate = id.is_none() && input.deactivate;
        let event_id: Option<i64> = if let Some(id) = id {
            sqlx::query_scalar("UPDATE events SET title = ?, occurs_at = ?, notes = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND role_id = ? AND role_id IN (SELECT id FROM roles WHERE user_id = ?) RETURNING id").bind(input.title).bind(input.occurs_at).bind(input.notes).bind(id).bind(role).bind(user).fetch_optional(&mut *tx).await?
        } else {
            sqlx::query_scalar("INSERT INTO events (role_id, title, occurs_at, notes) SELECT id, ?, ?, ? FROM roles WHERE id = ? AND user_id = ? RETURNING id").bind(input.title).bind(input.occurs_at).bind(input.notes).bind(role).bind(user).fetch_optional(&mut *tx).await?
        };
        let Some(id) = event_id else {
            return Ok(false);
        };
        sqlx::query("DELETE FROM event_people WHERE event_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for person in input.people {
            if !Person::attach(&mut tx, role, id, person).await? {
                return Ok(false);
            }
        }
        // Reattach submitted people before pruning: unchanged selections keep their records.
        Person::delete_unlinked(&mut tx, role).await?;
        if deactivate {
            sqlx::query("UPDATE roles SET active = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND user_id = ?")
                .bind(role).bind(user).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(true)
    }
    pub async fn delete(
        pool: &SqlitePool,
        user: i64,
        role: i64,
        id: i64,
    ) -> Result<bool, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM events WHERE id = ? AND role_id = ? AND role_id IN (SELECT id FROM roles WHERE user_id = ?)")
            .bind(id).bind(role).bind(user).execute(&mut *tx).await?.rows_affected() == 1;
        if deleted {
            Person::delete_unlinked(&mut tx, role).await?;
        }
        tx.commit().await?;
        Ok(deleted)
    }
}
