use crate::{
    AppState,
    controllers::auth::{forbidden, htmx, redirect, signed_in},
    error::AppError,
    models::{
        person::{Input, Person},
        role::Role,
    },
    views,
};
use axum::{
    Form,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

#[derive(serde::Deserialize)]
pub struct UpdateInput {
    csrf: String,
    #[serde(flatten)]
    details: Input,
}
fn missing() -> Response {
    (StatusCode::NOT_FOUND, views::layout::page("Person not found", maud::html! { h1 { "Person or application not found" } a href="/applied" { "Back to Applied" } })).into_response()
}
pub async fn edit_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role_id, id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    let Some(role) = Role::find(&state.pool, user.id, role_id).await? else {
        return Ok(missing());
    };
    let Some(person) = Person::find(&state.pool, user.id, role_id, id).await? else {
        return Ok(missing());
    };
    let input = Input {
        first_name: person.first_name,
        last_name: person.last_name,
        job_title: person.job_title,
        email: person.email,
        phone: person.phone,
        notes: person.notes,
        ..Input::default()
    };
    Ok(views::people::page(&user, &session.csrf, &role, id, &input, None).into_response())
}
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role_id, id)): Path<(i64, i64)>,
    Form(input): Form<UpdateInput>,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    let Some(role) = Role::find(&state.pool, user.id, role_id).await? else {
        return Ok(missing());
    };
    if Person::find(&state.pool, user.id, role_id, id)
        .await?
        .is_none()
    {
        return Ok(missing());
    }
    let validated = match input.details.validate() {
        Ok(value) => value,
        Err(error) => {
            let error = if input.details.first_name.trim().is_empty() {
                "Enter a first name."
            } else {
                error
            };
            let view = if htmx(&headers) {
                views::people::form(&input.details, &session.csrf, role_id, id, Some(error))
            } else {
                views::people::page(&user, &session.csrf, &role, id, &input.details, Some(error))
            };
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, view).into_response());
        }
    };
    if !Person::update(&state.pool, user.id, role_id, id, validated).await? {
        return Ok(missing());
    }
    Ok(redirect(&headers, &format!("/applied/{role_id}")))
}
