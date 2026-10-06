use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use jobtowel::{AppState, db, routes};
use tower::ServiceExt;

async fn app() -> (Router, sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite://{}",
        dir.path().join("test.db").display()
    ))
    .await
    .unwrap();
    db::migrate(&pool).await.unwrap();
    (
        routes::router(
            AppState {
                pool: pool.clone(),
                session_cookie_secure: true,
            },
            dir.path(),
        ),
        pool,
        dir,
    )
}
async fn request(app: &Router, path: &str, cookie: &str, body: Option<&str>, hx: bool) -> Response {
    let mut builder = Request::builder().uri(path).header("cookie", cookie);
    if hx {
        builder = builder.header("HX-Request", "true");
    }
    if body.is_some() {
        builder = builder
            .method("POST")
            .header("content-type", "application/x-www-form-urlencoded");
    }
    app.clone()
        .oneshot(
            builder
                .body(Body::from(body.unwrap_or("").to_owned()))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn text(response: Response) -> String {
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
fn cookie(response: &Response) -> String {
    let value = response.headers()["set-cookie"].to_str().unwrap();
    assert!(
        value.contains("HttpOnly") && value.contains("SameSite=Lax") && value.contains("Secure")
    );
    value.split(';').next().unwrap().into()
}
fn csrf(html: &str) -> String {
    html.split("name=\"csrf\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .into()
}
async fn entry(app: &Router, path: &str) -> (String, String) {
    let response = request(app, path, "", None, false).await;
    let cookie = cookie(&response);
    (cookie, csrf(&text(response).await))
}
async fn signup(app: &Router, username: &str, email: &str) -> String {
    let (guest, token) = entry(app, "/signup").await;
    let response = request(app, "/signup", &guest, Some(&format!("username={username}&name=Test+User&email={email}&password=long-password-123&csrf={token}")), false).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["location"], "/dashboard");
    cookie(&response)
}

#[tokio::test]
async fn signup_persists_hash_rotates_session_and_protects_dashboard() {
    let (app, pool, _dir) = app().await;
    for path in ["/", "/dashboard", "/account"] {
        let response = request(&app, path, "", None, false).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()["location"], "/login");
    }
    let signed = signup(&app, "tester", "test@example.com").await;
    let html = text(request(&app, "/dashboard", &signed, None, false).await).await;
    assert!(
        html.contains("Welcome, Test User.")
            && html.contains("Account")
            && html.contains("Log Out")
    );
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$") && !hash.contains("long-password-123"));
    let stored: String =
        sqlx::query_scalar("SELECT token_hash FROM sessions WHERE user_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!signed.contains(&stored));
    sqlx::query("UPDATE sessions SET expires_at = 0")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "/dashboard", &signed, None, false)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
}
#[tokio::test]
async fn htmx_validation_preserves_fields_never_echoes_password_and_checks_csrf() {
    let (app, pool, _dir) = app().await;
    let (guest, token) = entry(&app, "/signup").await;
    let response = request(
        &app,
        "/signup",
        &guest,
        Some(&format!(
            "username=ab&name=%3Cscript%3E&email=test@example.com&password=secret&csrf={token}"
        )),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let html = text(response).await;
    assert!(html.contains("id=\"auth-form\"") && !html.contains("<!DOCTYPE"));
    assert!(html.contains("&lt;script&gt;") && html.contains("value=\"ab\""));
    assert!(!html.contains("secret") && !html.contains("<script>"));
    assert_eq!(
        request(
            &app,
            "/signup",
            &guest,
            Some("username=tester&csrf=wrong"),
            true
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let response = request(
        &app,
        "/signup",
        &guest,
        Some(&format!(
            "username=tester&name=Test&email=bad&password=long-password-123&csrf={token}"
        )),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(text(response).await.contains("<!DOCTYPE"));
}
#[tokio::test]
async fn login_duplicates_logout_and_password_change() {
    let (app, pool, _dir) = app().await;
    let signed = signup(&app, "tester", "test@example.com").await;
    let (guest, token) = entry(&app, "/signup").await;
    let duplicate = request(&app, "/signup", &guest, Some(&format!("username=TESTER&name=Other&email=other@example.com&password=long-password-123&csrf={token}")), true).await;
    assert_eq!(duplicate.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let (guest, token) = entry(&app, "/login").await;
    let unknown = request(
        &app,
        "/login",
        &guest,
        Some(&format!("username=unknown&password=wrong&csrf={token}")),
        true,
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let invalid = request(
        &app,
        "/login",
        &guest,
        Some(&format!("username=tester&password=wrong&csrf={token}")),
        true,
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let login = request(
        &app,
        "/login",
        &guest,
        Some(&format!(
            "username=TESTER&password=long-password-123&csrf={token}"
        )),
        true,
    )
    .await;
    assert_eq!(login.headers()["HX-Redirect"], "/dashboard");
    let other = cookie(&login);
    assert_eq!(
        request(&app, "/dashboard", &guest, None, false)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    let html = text(request(&app, "/account", &signed, None, false).await).await;
    let token = csrf(&html);
    let fields = "name=%3CNew%3E&email=new@example.com&password=new-long-password";
    let invalid = request(
        &app,
        "/account",
        &signed,
        Some(&format!("{fields}&current_password=wrong&csrf={token}")),
        true,
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let response = request(
        &app,
        "/account",
        &signed,
        Some(&format!(
            "{fields}&current_password=long-password-123&csrf={token}"
        )),
        true,
    )
    .await;
    assert_eq!(response.headers()["HX-Redirect"], "/account?saved=1");
    let updated = cookie(&response);
    for old in [&signed, &other] {
        assert_eq!(
            request(&app, "/account", old, None, false).await.status(),
            StatusCode::SEE_OTHER
        );
    }
    let html = text(request(&app, "/account?saved=1", &updated, None, false).await).await;
    assert!(
        html.contains("&lt;New&gt;")
            && html.contains("new@example.com")
            && html.contains("has been updated")
    );
    let token = csrf(&html);
    assert_eq!(
        request(&app, "/logout", &updated, Some("csrf=wrong"), false)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = request(
        &app,
        "/logout",
        &updated,
        Some(&format!("csrf={token}")),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(
        response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        request(&app, "/dashboard", &updated, None, false)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    let (guest, token) = entry(&app, "/login").await;
    assert_eq!(
        request(
            &app,
            "/login",
            &guest,
            Some(&format!(
                "username=tester&password=long-password-123&csrf={token}"
            )),
            false
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        request(
            &app,
            "/login",
            &guest,
            Some(&format!(
                "username=tester&password=new-long-password&csrf={token}"
            )),
            false
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    db::migrate(&pool).await.unwrap();
}
#[tokio::test]
async fn name_only_edit_and_duplicate_email_are_safe() {
    let (app, pool, _dir) = app().await;
    let signed = signup(&app, "first", "first@example.com").await;
    signup(&app, "second", "second@example.com").await;
    let token = csrf(&text(request(&app, "/account", &signed, None, false).await).await);
    let duplicate = request(
        &app,
        "/account",
        &signed,
        Some(&format!(
            "name=Changed&email=second@example.com&csrf={token}"
        )),
        true,
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE username = 'first'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Test User");
    let response = request(
        &app,
        "/account",
        &signed,
        Some(&format!(
            "name=Changed&email=first@example.com&csrf={token}"
        )),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(
        text(request(&app, "/dashboard", &cookie(&response), None, false).await)
            .await
            .contains("Welcome, Changed.")
    );
}

#[tokio::test]
async fn csrf_tokens_are_session_bound_and_database_errors_are_safe() {
    let (app, pool, _dir) = app().await;
    let (guest, _) = entry(&app, "/signup").await;
    let (_, other_csrf) = entry(&app, "/signup").await;
    assert_eq!(
        request(
            &app,
            "/signup",
            &guest,
            Some(&format!("csrf={other_csrf}")),
            true
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    pool.close().await;
    let response = request(&app, "/signup", &guest, Some("name=draft"), true).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.headers()["HX-Retarget"], "#request-error");
    let html = text(response).await;
    assert!(!html.contains("pool closed"));
    assert!(html.contains("Please try again"));
}

#[tokio::test]
async fn retired_routes_are_gone_and_http_protections_remain() {
    let (app, pool, _dir) = app().await;
    assert_eq!(
        request(&app, "/demo", "", None, false).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, "/comments", "", Some(""), false)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/signup")
                .header("host", "localhost:3000")
                .header("origin", "https://other.example")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=Draft"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = request(
        &app,
        "/signup",
        "",
        Some(&format!("name={}", "a".repeat(40_000))),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let response = request(&app, "/login", "", None, false).await;
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["referrer-policy"], "same-origin");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn email_changes_need_no_password_but_password_changes_still_do() {
    let (app, pool, _dir) = app().await;
    let signed = signup(&app, "tester", "test@example.com").await;
    let token = csrf(&text(request(&app, "/account", &signed, None, false).await).await);
    let original_hash: String = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = request(
        &app,
        "/account",
        &signed,
        Some(&format!(
            "name=Test+User&email=updated@example.com&csrf={token}"
        )),
        true,
    )
    .await;
    assert_eq!(response.headers()["HX-Redirect"], "/account?saved=1");
    let updated = cookie(&response);
    let (email, hash): (String, String) = sqlx::query_as("SELECT email, password_hash FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(email, "updated@example.com");
    assert_eq!(hash, original_hash);
    let html = text(request(&app, "/account", &updated, None, false).await).await;
    let token = csrf(&html);
    let response = request(
        &app,
        "/account",
        &updated,
        Some(&format!(
            "name=Test+User&email=another@example.com&password=new-long-password&csrf={token}"
        )),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let html = text(response).await;
    assert!(html.contains("Enter your current password to change your password."));
    assert!(html.contains("value=\"another@example.com\""));
    let (email, hash): (String, String) = sqlx::query_as("SELECT email, password_hash FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(email, "updated@example.com");
    assert_eq!(hash, original_hash);
}
