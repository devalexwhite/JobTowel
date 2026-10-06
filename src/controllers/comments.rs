use crate::{
    AppState,
    error::AppError,
    models::comment::{Comment, NewComment},
    views,
};
use axum::{
    Form,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<NewComment>,
) -> Result<Response, AppError> {
    let htmx = headers
        .get("HX-Request")
        .is_some_and(|value| value == "true");
    let validated = match input.validate() {
        Ok(comment) => comment,
        Err(message) => {
            let comments = Comment::all(&state.pool).await?;
            let view = if htmx {
                views::comments::panel(&comments, &input.body, Some(message), false)
            } else {
                views::home::index(&comments, &input.body, Some(message), false)
            };
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, view).into_response());
        }
    };
    Comment::create(&state.pool, validated).await?;
    if htmx {
        let comments = Comment::all(&state.pool).await?;
        Ok(views::comments::panel(&comments, "", None, true).into_response())
    } else {
        // Post/redirect/get prevents reposting on refresh.
        Ok(Redirect::to("/").into_response())
    }
}
