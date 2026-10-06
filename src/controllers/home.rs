use crate::{AppState, error::AppError, models::comment::Comment, views};
use axum::extract::State;
use maud::Markup;

pub async fn index(State(state): State<AppState>) -> Result<Markup, AppError> {
    let comments = Comment::all(&state.pool).await?;
    Ok(views::home::index(&comments, "", None, false))
}
