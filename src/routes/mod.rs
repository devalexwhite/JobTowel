use crate::{AppState, controllers, views};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Request},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::path::Path;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};

pub fn router(state: AppState, asset_dir: impl AsRef<Path>) -> Router {
    Router::new()
        .route("/", get(controllers::auth::dashboard))
        .route("/dashboard", get(controllers::auth::dashboard))
        .route(
            "/signup",
            get(controllers::auth::signup_page).post(controllers::auth::signup),
        )
        .route(
            "/login",
            get(controllers::auth::login_page).post(controllers::auth::login),
        )
        .route(
            "/account",
            get(controllers::auth::account_page).post(controllers::auth::account),
        )
        .route("/logout", post(controllers::auth::logout))
        .nest_service("/assets", ServeDir::new(asset_dir))
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                views::layout::page(
                    "Page not found",
                    maud::html! { h1 { "Page not found" } a href="/" { "Back to home" } },
                ),
            )
        })
        .layer(DefaultBodyLimit::max(32 * 1024))
        .layer(middleware::from_fn(same_origin))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// Reject browser cross-origin writes. Origin-less clients (curl, tests) are allowed.
// Authenticated forms additionally validate session-bound CSRF tokens.
async fn same_origin(request: Request, next: Next) -> Response {
    if request.method() == axum::http::Method::POST {
        let headers = request.headers();
        let cross_site = headers
            .get("sec-fetch-site")
            .is_some_and(|value| value == "cross-site");
        let invalid_origin = headers.get(header::ORIGIN).is_some_and(|origin| {
            let origin = origin
                .to_str()
                .ok()
                .and_then(|value| value.parse::<axum::http::Uri>().ok());
            let host = headers
                .get(header::HOST)
                .and_then(|value| value.to_str().ok());
            !origin.is_some_and(|uri| {
                matches!(uri.scheme_str(), Some("http" | "https"))
                    && uri
                        .authority()
                        .is_some_and(|authority| Some(authority.as_str()) == host)
            })
        });
        if cross_site || invalid_origin {
            return (
                StatusCode::FORBIDDEN,
                "Cross-origin form submissions are not allowed.",
            )
                .into_response();
        }
    }
    next.run(request).await
}
