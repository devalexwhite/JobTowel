use crate::views::layout;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use maud::html;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("authentication operation failed")]
    Authentication,
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!(error = ?self, "request failed");
        let content = html! {
            div id="error-message" class="alert alert-danger" role="alert" {
                "We couldn't complete your request. Please try again."
            }
        };
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            [
                ("HX-Retarget", "#request-error"),
                ("HX-Reswap", "innerHTML"),
                ("HX-Reselect", "#error-message"),
            ],
            layout::page("Something went wrong", content),
        )
            .into_response()
    }
}
