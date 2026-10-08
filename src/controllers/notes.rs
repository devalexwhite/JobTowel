use crate::{
    AppState,
    controllers::auth::{forbidden, redirect, signed_in},
    error::AppError,
    views,
};
use axum::{
    Form,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

#[derive(serde::Deserialize)]
pub struct PreviewInput {
    csrf: String,
    notes: String,
}

pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<PreviewInput>,
) -> Result<Response, AppError> {
    let Some((session, _)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    if input.notes.chars().count() > 10000 {
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            maud::html! { p role="alert" { "Keep notes to 10,000 characters or fewer." } },
        )
            .into_response());
    }
    let content = if input.notes.trim().is_empty() {
        maud::html! { p class="text-body-secondary" { "Nothing to preview yet." } }
    } else {
        views::notes::render(&input.notes)
    };
    Ok(content.into_response())
}
