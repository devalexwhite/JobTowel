use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone, FromRow)]
pub struct Person {
    pub id: i64,
    pub role_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub job_title: String,
    pub email: String,
    pub phone: String,
    pub notes: String,
}
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(default)]
pub struct Input {
    #[serde(skip)]
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub job_title: String,
    pub email: String,
    pub phone: String,
    pub notes: String,
}

pub enum ValidatedInput {
    Existing(i64),
    New(Input),
}
impl Input {
    pub fn validate(&self) -> Result<ValidatedInput, &'static str> {
        if !self.id.is_empty() {
            return self
                .id
                .parse::<i64>()
                .ok()
                .filter(|id| *id > 0)
                .map(ValidatedInput::Existing)
                .ok_or("Choose an existing person from this application, or add a new person.");
        }
        let person = Input {
            id: String::new(),
            first_name: self.first_name.trim().into(),
            last_name: self.last_name.trim().into(),
            job_title: self.job_title.trim().into(),
            email: self.email.trim().into(),
            phone: self.phone.trim().into(),
            notes: self.notes.trim().into(),
        };
        if person.first_name.is_empty() {
            return Err("Enter a first name for each person, or remove their row.");
        }
        if [
            &person.first_name,
            &person.last_name,
            &person.job_title,
            &person.email,
            &person.phone,
        ]
        .iter()
        .any(|field| field.chars().count() > 200 || field.chars().any(char::is_control))
        {
            return Err(
                "Keep each person's name, job title, email, and phone to 200 characters or fewer, without control characters.",
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
        Ok(ValidatedInput::New(person))
    }
    pub fn from_person(person: &Person) -> Self {
        Self {
            id: person.id.to_string(),
            ..Self::default()
        }
    }
}
impl Person {
    pub async fn find(
        pool: &SqlitePool,
        user: i64,
        role: i64,
        id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT people.* FROM people JOIN roles ON roles.id = people.role_id WHERE roles.user_id = ? AND people.role_id = ? AND people.id = ?")
            .bind(user).bind(role).bind(id).fetch_optional(pool).await
    }
    pub async fn update(
        pool: &SqlitePool,
        user: i64,
        role: i64,
        id: i64,
        input: ValidatedInput,
    ) -> Result<bool, sqlx::Error> {
        let ValidatedInput::New(person) = input else {
            return Ok(false);
        };
        Ok(sqlx::query("UPDATE people SET first_name = ?, last_name = ?, job_title = ?, email = ?, phone = ?, notes = ? WHERE id = ? AND role_id = ? AND EXISTS (SELECT 1 FROM roles WHERE roles.id = people.role_id AND roles.user_id = ?)")
            .bind(person.first_name).bind(person.last_name).bind(person.job_title).bind(person.email).bind(person.phone).bind(person.notes)
            .bind(id).bind(role).bind(user).execute(pool).await?.rows_affected() == 1)
    }

    pub async fn list(pool: &SqlitePool, user: i64, role: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as("SELECT people.* FROM people JOIN roles ON roles.id = people.role_id WHERE roles.user_id = ? AND people.role_id = ? ORDER BY first_name COLLATE NOCASE, last_name COLLATE NOCASE, people.id")
            .bind(user).bind(role).fetch_all(pool).await
    }
    pub async fn delete_unlinked(
        tx: &mut Transaction<'_, Sqlite>,
        role: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM people WHERE role_id = ? AND NOT EXISTS (SELECT 1 FROM event_people WHERE person_id = people.id)")
            .bind(role).execute(&mut **tx).await?;
        Ok(())
    }
    pub async fn attach(
        tx: &mut Transaction<'_, Sqlite>,
        role: i64,
        event: i64,
        input: ValidatedInput,
    ) -> Result<bool, sqlx::Error> {
        let id = match input {
            ValidatedInput::Existing(id) => {
                let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM people WHERE id = ? AND role_id = ?)")
                    .bind(id).bind(role).fetch_one(&mut **tx).await?;
                if !exists { return Ok(false); }
                id
            }
            ValidatedInput::New(person) => sqlx::query_scalar("INSERT INTO people (role_id, first_name, last_name, job_title, email, phone, notes) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id")
                .bind(role).bind(person.first_name).bind(person.last_name).bind(person.job_title).bind(person.email).bind(person.phone).bind(person.notes).fetch_one(&mut **tx).await?,
        };
        sqlx::query("INSERT INTO event_people (event_id, person_id, role_id) VALUES (?, ?, ?) ON CONFLICT(event_id, person_id) DO NOTHING")
            .bind(event).bind(id).bind(role).execute(&mut **tx).await?;
        Ok(true)
    }
}
