use crate::{
    AppState,
    controllers::auth::{forbidden, htmx, redirect, signed_in},
    error::AppError,
    models::{
        event::{Event, Input},
        person::{self, Person},
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
fn missing() -> Response {
    (StatusCode::NOT_FOUND, views::layout::page("Event not found", maud::html! { h1 { "Event or application not found" } a href="/applied" { "Back to Applied" } })).into_response()
}
pub async fn view(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<i64>,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    let Some(role) = Role::find(&state.pool, user.id, role_id).await? else {
        return Ok(missing());
    };
    let mut timeline = Vec::new();
    for event in Event::list(&state.pool, user.id, role_id).await? {
        let people = event.people(&state.pool).await?;
        timeline.push((event, people));
    }
    Ok(views::events::timeline(&user, &session.csrf, &role, &timeline).into_response())
}
pub async fn new_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role): Path<i64>,
) -> Result<Response, AppError> {
    show(state, headers, role, None, false).await
}
pub async fn edit_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role, id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    show(state, headers, role, Some(id), false).await
}
pub async fn delete_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role, id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    show(state, headers, role, Some(id), true).await
}
async fn show(
    state: AppState,
    headers: HeaderMap,
    role_id: i64,
    id: Option<i64>,
    deleting: bool,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    let Some(role) = Role::find(&state.pool, user.id, role_id).await? else {
        return Ok(missing());
    };
    let mut input = if let Some(id) = id {
        let Some(event) = Event::find(&state.pool, user.id, role_id, id).await? else {
            return Ok(missing());
        };
        Input::from_event(&event, &event.people(&state.pool).await?, user.timezone())
    } else {
        Input::for_new(user.timezone())
    };
    input.available_people = Person::list(&state.pool, user.id, role_id).await?;
    Ok(
        views::events::page(&user, &session.csrf, &role, &input, id, deleting, None)
            .into_response(),
    )
}
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role): Path<i64>,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    save(state, headers, role, None, input, false).await
}
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role, id)): Path<(i64, i64)>,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    save(state, headers, role, Some(id), input, false).await
}
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role, id)): Path<(i64, i64)>,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    save(state, headers, role, Some(id), input, true).await
}
async fn save(
    state: AppState,
    headers: HeaderMap,
    role_id: i64,
    id: Option<i64>,
    mut input: Input,
    deleting: bool,
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
    if let Some(id) = id
        && Event::find(&state.pool, user.id, role_id, id)
            .await?
            .is_none()
    {
        return Ok(missing());
    }
    input.available_people = Person::list(&state.pool, user.id, role_id).await?;
    if !deleting && !input.people_action.is_empty() {
        let mut error = None;
        if input.people_action == "add" {
            if input.people.len() < 20 {
                input.people.push(person::Input::default());
            } else {
                error = Some("Add up to 20 people per event.");
            }
        } else if let Some(index) = input
            .people_action
            .strip_prefix("remove:")
            .and_then(|index| index.parse::<usize>().ok())
            && index < input.people.len()
        {
            input.people.remove(index);
        }
        let view = if htmx(&headers) {
            views::events::form(&input, &session.csrf, role_id, id, user.timezone(), error)
        } else {
            views::events::page(&user, &session.csrf, &role, &input, id, false, error)
        };
        return Ok((
            if error.is_some() {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                StatusCode::OK
            },
            view,
        )
            .into_response());
    }
    let saved = if deleting {
        Event::delete(&state.pool, user.id, role_id, id.unwrap()).await?
    } else {
        let membership =
            input.people.iter().all(|person| {
                person.id.is_empty()
                    || person.id.parse::<i64>().ok().is_some_and(|id| {
                        input.available_people.iter().any(|person| person.id == id)
                    })
            });
        let validated = match if membership {
            input.validate(user.timezone())
        } else {
            Err("Choose an existing person from this application, or add a new person.")
        } {
            Ok(value) => value,
            Err(error) => {
                let view = if htmx(&headers) {
                    views::events::form(
                        &input,
                        &session.csrf,
                        role_id,
                        id,
                        user.timezone(),
                        Some(error),
                    )
                } else {
                    views::events::page(&user, &session.csrf, &role, &input, id, false, Some(error))
                };
                return Ok((StatusCode::UNPROCESSABLE_ENTITY, view).into_response());
            }
        };
        if id.is_none()
            && validated.is_removal()
            && !["keep", "inactive"].contains(&input.removal_status.as_str())
        {
            input.status_prompt = true;
            let view = if htmx(&headers) {
                views::events::form(&input, &session.csrf, role_id, id, user.timezone(), None)
            } else {
                views::events::page(&user, &session.csrf, &role, &input, id, false, None)
            };
            return Ok(view.into_response());
        }
        Event::save(&state.pool, user.id, role_id, id, validated).await?
    };
    if !saved {
        return Ok(missing());
    }
    Ok(redirect(&headers, &format!("/applied/{role_id}")))
}
