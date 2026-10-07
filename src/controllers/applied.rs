use crate::{
    AppState,
    controllers::auth::{forbidden, htmx, redirect, signed_in},
    error::AppError,
    models::role::{AppliedRole, Input, Role},
    views,
};
use axum::{
    Form,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

fn missing() -> Response {
    (
        StatusCode::NOT_FOUND,
        views::layout::page(
            "Application not found",
            maud::html! { h1 { "Application not found" } a href="/applied" { "Back to Applied" } },
        ),
    )
        .into_response()
}

pub async fn index(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    Ok(views::applied::index(
        &user,
        &session.csrf,
        &AppliedRole::list(&state.pool, user.id).await?,
    )
    .into_response())
}
pub async fn new_page(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    show(state, headers, None).await
}
pub async fn edit_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    show(state, headers, Some(id)).await
}
async fn show(state: AppState, headers: HeaderMap, id: Option<i64>) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    let input = if let Some(id) = id {
        let Some(role) = Role::find(&state.pool, user.id, id).await? else {
            return Ok(missing());
        };
        Input::from(&role)
    } else {
        Input::default()
    };
    Ok(views::applied::form_page(
        &user,
        &session.csrf,
        views::applied::form(&input, &session.csrf, id, None),
        id.is_some(),
    )
    .into_response())
}
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    save(state, headers, None, input).await
}
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    save(state, headers, Some(id), input).await
}
async fn save(
    state: AppState,
    headers: HeaderMap,
    id: Option<i64>,
    input: Input,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    if let Some(id) = id
        && Role::find(&state.pool, user.id, id).await?.is_none()
    {
        return Ok(missing());
    }
    let validated = match input.validate() {
        Ok(input) => input,
        Err(error) => {
            let form = views::applied::form(&input, &session.csrf, id, Some(error));
            let view = if htmx(&headers) {
                form
            } else {
                views::applied::form_page(&user, &session.csrf, form, id.is_some())
            };
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, view).into_response());
        }
    };
    if let Some(id) = id {
        if !Role::update(&state.pool, user.id, id, validated).await? {
            return Ok(missing());
        }
    } else {
        Role::create(&state.pool, user.id, validated).await?;
    }
    Ok(redirect(&headers, "/applied"))
}
pub async fn delete_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    let Some(role) = Role::find(&state.pool, user.id, id).await? else {
        return Ok(missing());
    };
    Ok(views::applied::confirmation(&user, &session.csrf, &role).into_response())
}
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    if !Role::delete(&state.pool, user.id, id).await? {
        return Ok(missing());
    }
    if htmx(&headers)
        && headers
            .get("HX-Target")
            .is_some_and(|v| v == "applied-roles")
    {
        Ok(views::applied::table(
            &AppliedRole::list(&state.pool, user.id).await?,
            &session.csrf,
            true,
        )
        .into_response())
    } else {
        Ok(redirect(&headers, "/applied"))
    }
}
