use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use jobtowel::{AppState, db, models::comment::Comment, routes};
use tower::ServiceExt;

async fn app() -> (Router, sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", dir.path().join("test.db").display());
    let pool = db::connect(&url).await.unwrap();
    db::migrate(&pool).await.unwrap();
    (
        routes::router(AppState { pool: pool.clone() }, dir.path()),
        pool,
        dir,
    )
}

fn post(body: &str, htmx: bool) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/comments")
        .header("content-type", "application/x-www-form-urlencoded");
    if htmx {
        builder = builder.header("HX-Request", "true");
    }
    builder.body(Body::from(body.to_owned())).unwrap()
}

async fn text(response: axum::response::Response) -> String {
    String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn index_shows_form_empty_state_and_local_assets() {
    let (app, _, _dir) = app().await;
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "text/html; charset=utf-8"
    );
    let html = text(response).await;
    assert!(html.contains("hx-post=\"/comments\""));
    assert!(html.contains("No comments yet"));
    assert!(html.contains("/assets/app.css"));
    assert!(html.contains("/assets/app.js"));
}

#[tokio::test]
async fn htmx_post_persists_and_returns_all_comments_newest_first_escaped() {
    let (app, pool, _dir) = app().await;
    app.clone().oneshot(post("body=first", true)).await.unwrap();
    let response = app
        .clone()
        .oneshot(post(
            "body=%20%3Cscript%3Ealert%281%29%3C%2Fscript%3E%20",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = text(response).await;
    assert!(html.contains("id=\"comments-panel\""));
    assert!(!html.contains("<!DOCTYPE"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
    assert!(html.find("&lt;script&gt;").unwrap() < html.find("first").unwrap());
    assert!(html.contains("Your comment was posted."));
    let comments = Comment::all(&pool).await.unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].body, "<script>alert(1)</script>");
    assert!(comments[0].created_at.ends_with('Z'));
}

#[tokio::test]
async fn plain_form_redirects_and_reload_shows_comment() {
    let (app, _, _dir) = app().await;
    let response = app
        .clone()
        .oneshot(post("body=Hello", false))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["location"], "/");
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(text(response).await.contains("Hello"));
}

#[tokio::test]
async fn invalid_comments_return_422_without_inserting_and_preserve_draft() {
    let (app, pool, _dir) = app().await;
    for body in [
        "body=%20%0A%20".to_owned(),
        format!("body={}", "a".repeat(2001)),
        "body=test%00comment".to_owned(),
        String::new(),
    ] {
        let response = app.clone().oneshot(post(&body, true)).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(text(response).await.contains("is-invalid"));
    }
    assert!(Comment::all(&pool).await.unwrap().is_empty());
    let response = app
        .oneshot(post(&format!("body={}", "x".repeat(2001)), false))
        .await
        .unwrap();
    let html = text(response).await;
    assert!(html.contains("<!DOCTYPE"));
    assert!(html.contains(&"x".repeat(2001)));
}

#[tokio::test]
async fn unicode_boundary_is_valid_and_migrations_are_repeatable() {
    let (app, pool, _dir) = app().await;
    db::migrate(&pool).await.unwrap();
    let response = app
        .oneshot(post(&format!("body={}", "%F0%9F%98%80".repeat(2000)), true))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        Comment::all(&pool).await.unwrap()[0].body.chars().count(),
        2000
    );
}

#[tokio::test]
async fn cross_origin_writes_and_oversized_requests_are_rejected() {
    let (app, pool, _dir) = app().await;
    let mut request = post("body=test", true);
    request
        .headers_mut()
        .insert("host", "localhost:3000".parse().unwrap());
    request
        .headers_mut()
        .insert("origin", "https://other.example".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut request = post("body=allowed", true);
    request
        .headers_mut()
        .insert("host", "localhost:3000".parse().unwrap());
    request
        .headers_mut()
        .insert("origin", "http://localhost:3000".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
    let response = app
        .oneshot(post(&format!("body={}", "a".repeat(40_000)), true))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(Comment::all(&pool).await.unwrap().len(), 1);
}

#[tokio::test]
async fn database_failures_are_generic_and_keep_draft_in_htmx_form() {
    let (app, pool, _dir) = app().await;
    pool.close().await;
    let response = app.oneshot(post("body=test", true)).await.unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.headers()["HX-Retarget"], "#request-error");
    let html = text(response).await;
    assert!(html.contains("Please try again"));
    assert!(!html.contains("pool closed"));
}

#[tokio::test]
async fn migrations_and_comments_survive_reopening_the_database() {
    let (app, pool, dir) = app().await;
    app.oneshot(post("body=persisted", true)).await.unwrap();
    pool.close().await;
    let reopened = db::connect(&format!(
        "sqlite://{}",
        dir.path().join("test.db").display()
    ))
    .await
    .unwrap();
    db::migrate(&reopened).await.unwrap();
    assert_eq!(Comment::all(&reopened).await.unwrap()[0].body, "persisted");
}
