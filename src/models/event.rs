use serde::{Deserialize, Deserializer};
use sqlx::{FromRow, SqlitePool};
use std::collections::{BTreeMap, HashMap};

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
#[derive(Debug, Default, Clone, FromRow)]
pub struct Person {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub notes: String,
}
#[derive(Default)]
pub struct Input {
    pub title: String,
    pub occurs_at: String,
    pub notes: String,
    pub people: Vec<Person>,
    pub people_action: String,
    pub csrf: String,
}
// Indexed flat fields support ordinary URL-encoded browser forms and HTMX alike.
impl<'de> Deserialize<'de> for Input {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut fields = HashMap::<String, String>::deserialize(deserializer)?;
        let mut input = Self {
            title: fields.remove("title").unwrap_or_default(),
            occurs_at: fields.remove("occurs_at").unwrap_or_default(),
            notes: fields.remove("notes").unwrap_or_default(),
            csrf: fields.remove("csrf").unwrap_or_default(),
            people_action: fields.remove("people_action").unwrap_or_default(),
            ..Self::default()
        };
        let mut people = BTreeMap::<usize, Person>::new();
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
                "first_name" => person.first_name = value,
                "last_name" => person.last_name = value,
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
    people: Vec<Person>,
}
impl Input {
    pub fn validate(&self) -> Result<ValidatedInput, &'static str> {
        let title = self.title.trim();
        if title.is_empty() || title.chars().count() > 200 || title.chars().any(char::is_control) {
            return Err("Enter an event title between 1 and 200 characters.");
        }
        if !valid_datetime(&self.occurs_at) {
            return Err("Enter a valid date and time in UTC.");
        }
        if self.notes.chars().count() > 10000 {
            return Err("Keep notes to 10,000 characters or fewer.");
        }
        if self.people.len() > 20 {
            return Err("Add up to 20 people per event.");
        }
        let mut people = Vec::new();
        for person in &self.people {
            let person = Person {
                first_name: person.first_name.trim().into(),
                last_name: person.last_name.trim().into(),
                email: person.email.trim().into(),
                phone: person.phone.trim().into(),
                notes: person.notes.trim().into(),
            };
            if person.first_name.is_empty() {
                return Err("Enter a first name for each person, or remove their row.");
            }
            if [
                &person.first_name,
                &person.last_name,
                &person.email,
                &person.phone,
            ]
            .iter()
            .any(|field| field.chars().count() > 200 || field.chars().any(char::is_control))
            {
                return Err(
                    "Keep each person's name, email, and phone to 200 characters or fewer, without control characters.",
                );
            }
            if person.notes.chars().count() > 10000 {
                return Err("Keep each person's notes to 10,000 characters or fewer.");
            }
            let email = &person.email;
            if !email.is_empty()
                && (!email.split_once('@').is_some_and(|(name, host)| {
                    !name.is_empty()
                        && host.contains('.')
                        && !host.starts_with('.')
                        && !host.ends_with('.')
                        && !host.contains('@')
                }) || email.chars().any(char::is_whitespace))
            {
                return Err("Enter a valid email for each person, or leave their email blank.");
            }
            people.push(person);
        }
        Ok(ValidatedInput {
            title: title.into(),
            occurs_at: format!("{}:00.000Z", self.occurs_at),
            notes: self.notes.trim().into(),
            people,
        })
    }
    pub fn from_event(event: &Event, people: &[Person]) -> Self {
        Self {
            title: event.title.clone(),
            occurs_at: event.occurs_at.chars().take(16).collect(),
            notes: event.notes.clone(),
            people: people.to_vec(),
            people_action: String::new(),
            csrf: String::new(),
        }
    }
}
fn valid_datetime(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 16
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b.iter()
            .enumerate()
            .any(|(i, c)| ![4, 7, 10, 13].contains(&i) && !c.is_ascii_digit())
    {
        return false;
    }
    let number = |range: std::ops::Range<usize>| value[range].parse::<u32>().unwrap_or(0);
    let (year, month, day, hour, minute) = (
        number(0..4),
        number(5..7),
        number(8..10),
        number(11..13),
        number(14..16),
    );
    let days = match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    year > 0 && day > 0 && day <= days && hour < 24 && minute < 60
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
            "SELECT first_name, last_name, email, phone, notes FROM event_people WHERE event_id = ? ORDER BY id",
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
            sqlx::query(
                "INSERT INTO event_people (event_id, first_name, last_name, email, phone, notes) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(person.first_name)
            .bind(person.last_name)
            .bind(person.email)
            .bind(person.phone)
            .bind(person.notes)
            .execute(&mut *tx)
            .await?;
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
        Ok(sqlx::query("DELETE FROM events WHERE id = ? AND role_id = ? AND role_id IN (SELECT id FROM roles WHERE user_id = ?)").bind(id).bind(role).bind(user).execute(pool).await?.rows_affected() == 1)
    }
}
