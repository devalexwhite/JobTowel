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
async fn timeline_distinguishes_future_events_by_instant() {
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "futureevents", "future-events@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    request(
        &app,
        "/applied/new",
        &owner,
        Some(&format!("csrf={token}&role_title=Engineer&company=Example")),
        false,
    )
    .await;
    let role: i64 = sqlx::query_scalar("SELECT id FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    let now = chrono::Utc::now();
    for (title, instant) in [
        ("Earlier", now - chrono::Duration::minutes(1)),
        ("Upcoming", now + chrono::Duration::minutes(1)),
    ] {
        sqlx::query("INSERT INTO events (role_id, title, occurs_at, notes) VALUES (?, ?, ?, '')")
            .bind(role)
            .bind(title)
            .bind(instant.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
            .execute(&pool)
            .await
            .unwrap();
    }
    let response = request(&app, &format!("/applied/{role}"), &owner, None, false).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = text(response).await;
    assert_eq!(html.matches("event-future").count(), 1);
    for item in html.split("<li class=\"position-relative").skip(1) {
        assert_eq!(item.contains("event-future"), item.contains("Upcoming"));
    }
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

#[tokio::test]
async fn applied_crud_is_owned_validated_and_tracks_record_timestamps() {
    use jobtowel::models::role::Role;
    let (app, pool, _dir) = app().await;
    for path in [
        "/applied",
        "/applied/new",
        "/applied/1/edit",
        "/applied/1/delete",
    ] {
        for body in [None, Some("csrf=bad")] {
            if path == "/applied" && body.is_some() {
                continue;
            }
            let response = request(&app, path, "", body, false).await;
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(response.headers()["location"], "/login");
        }
    }
    let owner = signup(&app, "roleowner", "roleowner@example.com").await;
    let other = signup(&app, "roleother", "roleother@example.com").await;
    let owner_html = text(request(&app, "/applied/new", &owner, None, false).await).await;
    let token = csrf(&owner_html);
    let other_token = csrf(&text(request(&app, "/applied/new", &other, None, false).await).await);
    let draft = format!(
        "csrf={token}&role_title=Engineer&company=Example&posting_url=javascript%3Aalert%281%29&role_description=%3Cscript%3Edraft%3C%2Fscript%3E&user_id=999"
    );
    for hx in [false, true] {
        let invalid = request(&app, "/applied/new", &owner, Some(&draft), hx).await;
        assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html = text(invalid).await;
        assert_eq!(html.contains("<!DOCTYPE html>"), !hx);
        assert!(
            html.contains("id=\"role-form\"")
                && html.contains("value=\"Engineer\"")
                && html.contains("&lt;script&gt;draft&lt;/script&gt;")
        );
    }
    assert_eq!(
        request(
            &app,
            "/applied/new",
            &owner,
            Some("csrf=bad&role_title=Engineer&company=Example"),
            true
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    for extra in [
        "role_title=&company=Example".to_owned(),
        format!("role_title={}&company=Example", "a".repeat(201)),
        "role_title=Engineer&company=".to_owned(),
        format!(
            "role_title=Engineer&company=Example&role_description={}",
            "x".repeat(10001)
        ),
    ] {
        assert_eq!(
            request(
                &app,
                "/applied/new",
                &owner,
                Some(&format!("csrf={token}&{extra}")),
                true
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let valid = format!(
        "csrf={token}&role_title=%3Cb%3EEngineer%3C%2Fb%3E&company=Example&posting_url=https%3A%2F%2Fexample.com%2Fjobs&role_description=Draft&user_id=999&created_at=old"
    );
    let created = request(&app, "/applied/new", &owner, Some(&valid), false).await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);
    assert_eq!(created.headers()["location"], "/applied");
    let owner_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'roleowner'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let role = Role::list(&pool, owner_id).await.unwrap().remove(0);
    assert_eq!(role.user_id, owner_id);
    assert_eq!(role.created_at, role.updated_at);
    assert_ne!(role.created_at, "old");
    let id = role.id;
    let listing = text(request(&app, "/applied", &owner, None, false).await).await;
    assert!(
        listing.contains("&lt;b&gt;Engineer&lt;/b&gt;")
            && listing.contains("hx-confirm=")
            && listing.contains("hx-post=")
    );
    let other_listing = text(request(&app, "/applied", &other, None, false).await).await;
    assert!(!other_listing.contains("Engineer"));
    for path in [
        format!("/applied/{id}/edit"),
        format!("/applied/{id}/delete"),
    ] {
        assert_eq!(
            request(&app, &path, &other, None, false).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &app,
                &path,
                &other,
                Some(&format!(
                    "csrf={other_token}&role_title=Stolen&company=Other"
                )),
                true
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    let confirmation =
        text(request(&app, &format!("/applied/{id}/delete"), &owner, None, false).await).await;
    assert!(
        confirmation.contains("Delete application?") && confirmation.contains("method=\"post\"")
    );
    assert!(Role::find(&pool, owner_id, id).await.unwrap().is_some());
    sqlx::query("UPDATE roles SET updated_at = '2000-01-01T00:00:00.000Z' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let edit = request(
        &app,
        &format!("/applied/{id}/edit"),
        &owner,
        Some(&format!(
            "csrf={token}&role_title=Updated&company=Example&role_description=Revised"
        )),
        true,
    )
    .await;
    assert_eq!(edit.headers()["HX-Redirect"], "/applied");
    let updated = Role::find(&pool, owner_id, id).await.unwrap().unwrap();
    assert_eq!(updated.created_at, role.created_at);
    assert!(updated.updated_at.as_str() > "2000-01-01T00:00:00.000Z");
    assert_eq!(updated.role_title, "Updated");
    assert_eq!(updated.role_description, "Revised");
    assert_eq!(
        request(
            &app,
            &format!("/applied/{id}/delete"),
            &owner,
            Some("csrf=bad"),
            true
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/applied/{id}/delete"))
                .header("cookie", &owner)
                .header("HX-Request", "true")
                .header("HX-Target", "applied-roles")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("csrf={token}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = text(response).await;
    assert!(
        html.contains("id=\"applied-roles\"")
            && html.contains("No applications match this status")
            && !html.contains("<!DOCTYPE")
    );
    assert!(Role::find(&pool, owner_id, id).await.unwrap().is_none());
    // A normal form deletes with post/redirect/get as well.
    request(&app, "/applied/new", &owner, Some(&valid), false).await;
    let next = Role::list(&pool, owner_id).await.unwrap().remove(0);
    let deleted = request(
        &app,
        &format!("/applied/{}/delete", next.id),
        &owner,
        Some(&format!("csrf={token}")),
        false,
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::SEE_OTHER);
    assert_eq!(deleted.headers()["location"], "/applied");
}

#[tokio::test]
async fn applied_shows_latest_event_by_date_with_ties_and_empty_timelines() {
    use jobtowel::models::role::{AppliedRole, Input, Role};
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "latestowner", "latest@example.com").await;
    let other = signup(&app, "latestother", "otherlatest@example.com").await;
    let user_id = sqlx::query_scalar("SELECT id FROM users WHERE username = 'latestowner'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let role = Role::create(
        &pool,
        user_id,
        Input {
            role_title: "Engineer".into(),
            company: "Example".into(),
            ..Input::default()
        }
        .validate()
        .unwrap(),
    )
    .await
    .unwrap();
    let listing = text(request(&app, "/applied", &owner, None, false).await).await;
    assert!(listing.contains(">Movement</th>"));
    assert!(listing.contains("<td class=\"text-break\">Applied<time"));
    let latest = AppliedRole::list(&pool, user_id).await.unwrap().remove(0);
    assert!(listing.contains(&format!(
        ">{}</time>",
        jobtowel::models::dates::display(latest.last_event_at.as_deref().unwrap(), "UTC")
    )));
    sqlx::query("DELETE FROM events WHERE role_id = ?")
        .bind(role.id)
        .execute(&pool)
        .await
        .unwrap();
    let listing = text(request(&app, "/applied", &owner, None, false).await).await;
    assert!(listing.contains("<td class=\"text-break\">No events</td>"));
    for (title, date) in [
        ("Interview", "2027-01-02T12:00:00.000Z"),
        ("<b>Offer</b>", "2027-01-02T12:00:00.000Z"),
        ("Added later but older", "2027-01-01T12:00:00.000Z"),
    ] {
        sqlx::query("INSERT INTO events (role_id, title, occurs_at) VALUES (?, ?, ?)")
            .bind(role.id)
            .bind(title)
            .bind(date)
            .execute(&pool)
            .await
            .unwrap();
    }
    let listing = text(request(&app, "/applied", &owner, None, false).await).await;
    assert!(listing.contains("<td class=\"text-break\">&lt;b&gt;Offer&lt;/b&gt;<time"));
    assert!(!listing.contains("Added later but older"));
    assert!(!listing.contains("<b>Offer</b>"));
    assert_eq!(AppliedRole::list(&pool, user_id).await.unwrap().len(), 1);
    let other_listing = text(request(&app, "/applied", &other, None, false).await).await;
    assert!(!other_listing.contains("Offer"));
    sqlx::query("DELETE FROM events WHERE title = ?")
        .bind("<b>Offer</b>")
        .execute(&pool)
        .await
        .unwrap();
    let listing = text(request(&app, "/applied", &owner, None, false).await).await;
    assert!(listing.contains("<td class=\"text-break\">Interview<time"));
    sqlx::query("DELETE FROM events WHERE role_id = ? AND title != 'Interview'")
        .bind(role.id)
        .execute(&pool)
        .await
        .unwrap();
    for modifier in [
        "0 days", "-1 day", "-2 days", "-7 days", "-30 days", "+2 days",
    ] {
        sqlx::query("UPDATE events SET occurs_at = strftime('%Y-%m-%dT00:00:00.000Z', 'now', ?) WHERE role_id = ?")
            .bind(modifier).bind(role.id).execute(&pool).await.unwrap();
        let listing = text(request(&app, "/applied", &owner, None, false).await).await;
        let latest = AppliedRole::list(&pool, user_id).await.unwrap().remove(0);
        let date = latest.last_event_at.unwrap();
        assert!(listing.contains(&format!(
            ">{}</time>",
            jobtowel::models::dates::display(&date, "UTC")
        )));
        assert!(listing.contains(&format!("datetime=\"{date}\"")));
    }
}

#[tokio::test]
async fn active_application_days_use_earliest_event_and_profile_calendar() {
    use chrono::{Duration, SecondsFormat, Utc};
    use jobtowel::models::{
        dates,
        role::{AppliedRole, Input, Role},
    };

    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "durationowner", "duration@example.com").await;
    let user_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'durationowner'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET timezone = 'Pacific/Kiritimati' WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let role = Role::create(
        &pool,
        user_id,
        Input {
            role_title: "Duration Engineer".into(),
            company: "Example".into(),
            ..Input::default()
        }
        .validate()
        .unwrap(),
    )
    .await
    .unwrap();
    sqlx::query("DELETE FROM events WHERE role_id = ?")
        .bind(role.id)
        .execute(&pool)
        .await
        .unwrap();
    let empty = text(request(&app, "/applied", &owner, None, false).await).await;
    assert!(empty.contains("<td>Active</td>"));
    let now = Utc::now();
    // Insert the newer event first to distinguish event order from insertion order.
    sqlx::query("INSERT INTO events (role_id, title, occurs_at) VALUES (?, 'Latest', ?)")
        .bind(role.id)
        .bind(now.to_rfc3339_opts(SecondsFormat::Millis, true))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO events (role_id, title, occurs_at) VALUES (?, 'First', ?)")
        .bind(role.id)
        .bind(now.to_rfc3339_opts(SecondsFormat::Millis, true))
        .execute(&pool)
        .await
        .unwrap();
    for days in [0, 1, 7] {
        let local_date = (now.with_timezone(&dates::zone("Pacific/Kiritimati"))
            - Duration::days(days))
        .date_naive();
        let first =
            dates::to_utc(&format!("{local_date}T00:00"), "Pacific/Kiritimati", "").unwrap();
        sqlx::query("UPDATE events SET occurs_at = ? WHERE role_id = ? AND title = 'First'")
            .bind(&first)
            .bind(role.id)
            .execute(&pool)
            .await
            .unwrap();
        let application = AppliedRole::list(&pool, user_id).await.unwrap().remove(0);
        assert_eq!(application.first_event_at.as_deref(), Some(first.as_str()));
        assert_eq!(application.active_days, Some(days));
        let subtitle = format!(
            "<td>Active<span class=\"d-block small text-body-secondary\">{days} {}</span></td>",
            if days == 1 { "day" } else { "days" }
        );
        for htmx in [false, true] {
            let listing =
                text(request(&app, "/applied?status=all", &owner, None, htmx).await).await;
            assert!(listing.contains(&subtitle));
        }
    }
    sqlx::query("UPDATE roles SET active = 0 WHERE id = ?")
        .bind(role.id)
        .execute(&pool)
        .await
        .unwrap();
    let inactive = text(request(&app, "/applied?status=all", &owner, None, false).await).await;
    assert!(inactive.contains("<td>Inactive</td>"));
}

#[tokio::test]
async fn applied_internal_errors_preserve_drafts_and_hide_database_details() {
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "brokenroles", "brokenroles@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    sqlx::query("DROP TABLE roles")
        .execute(&pool)
        .await
        .unwrap();
    let response = request(
        &app,
        "/applied/new",
        &owner,
        Some(&format!("csrf={token}&role_title=Engineer&company=Example")),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.headers()["HX-Retarget"], "#request-error");
    let html = text(response).await;
    assert!(html.contains("Please try again") && !html.contains("no such table"));
}

#[tokio::test]
async fn events_crud_timeline_validation_and_ownership() {
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "eventowner", "events@example.com").await;
    let other = signup(&app, "eventother", "other-events@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    let other_token = csrf(&text(request(&app, "/applied/new", &other, None, false).await).await);
    request(
        &app,
        "/applied/new",
        &owner,
        Some(&format!("csrf={token}&role_title=Engineer&company=Example")),
        false,
    )
    .await;
    let role: i64 = sqlx::query_scalar("SELECT id FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    let applied: (String, String) = sqlx::query_as("SELECT title, occurs_at FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let created: String = sqlx::query_scalar("SELECT created_at FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(applied, ("Applied".into(), created));
    sqlx::query("UPDATE events SET occurs_at = '2026-10-01T12:00:00.000Z'")
        .execute(&pool)
        .await
        .unwrap();
    let base = format!("/applied/{role}");
    let new = format!("{base}/events/new");
    let before: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M', 'now')")
        .fetch_one(&pool)
        .await
        .unwrap();
    let new_page = text(request(&app, &new, &owner, None, false).await).await;
    let after: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M', 'now')")
        .fetch_one(&pool)
        .await
        .unwrap();
    let default_date = new_page
        .split("name=\"occurs_at\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(default_date >= before.as_str() && default_date <= after.as_str());
    assert_eq!(default_date.len(), 16);
    assert_eq!(
        request(&app, &new, "", None, false).await.headers()["location"],
        "/login"
    );
    assert_eq!(
        request(&app, &base, &other, None, false).await.status(),
        StatusCode::NOT_FOUND
    );
    let draft = format!(
        "csrf={token}&title=Round+3&occurs_at=2026-02-30T14%3A30&notes=%3Cscript%3Edraft%3C%2Fscript%3E&person_0_first_name=John&person_0_last_name=Smith&person_0_job_title=++%3CHiring+manager%3E++&person_0_email=john%40example.com&person_0_phone=%2B1+555+1234&person_0_notes=Manager"
    );
    for hx in [false, true] {
        let response = request(&app, &new, &owner, Some(&draft), hx).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html = text(response).await;
        assert_eq!(html.contains("<!DOCTYPE"), !hx);
        assert!(
            html.contains("id=\"event-form\"")
                && html.contains("name=\"occurs_at\" value=\"2026-02-30T14:30\"")
                && html.contains("&lt;script&gt;draft&lt;/script&gt;")
                && html.contains("value=\"John\"")
                && html.contains("value=\"Smith\"")
                && html.contains("value=\"  &lt;Hiring manager&gt;  \"")
                && html.contains("value=\"john@example.com\"")
        );
    }
    assert_eq!(
        request(&app, &new, &owner, Some("csrf=bad"), true)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    // Row actions preserve drafts and never persist the event, even without JavaScript.
    for hx in [false, true] {
        let response = request(
            &app,
            &new,
            &owner,
            Some(&format!("{draft}&people_action=add")),
            hx,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let html = text(response).await;
        assert_eq!(html.contains("<!DOCTYPE"), !hx);
        assert!(html.contains("name=\"person_1_first_name\"") && html.contains("value=\"John\""));
        let response = request(
            &app,
            &new,
            &owner,
            Some(&format!(
                "{draft}&person_1_first_name=Jane&people_action=remove%3A0"
            )),
            hx,
        )
        .await;
        let html = text(response).await;
        assert!(html.contains("name=\"person_0_first_name\"") && html.contains("value=\"Jane\""));
        assert!(!html.contains("value=\"John\"") && !html.contains("name=\"person_1_first_name\""));
        assert!(html.contains("&lt;script&gt;draft&lt;/script&gt;"));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let missing_name = draft
        .replace("person_0_first_name=John", "person_0_first_name=")
        .replace("2026-02-30", "2026-10-08");
    let response = request(&app, &new, &owner, Some(&missing_name), true).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        text(response)
            .await
            .contains("Enter a first name for each person")
    );
    for title in ["x".repeat(201), "bad%00title".into()] {
        let invalid = draft
            .replace("2026-02-30", "2026-10-08")
            .replace("++%3CHiring+manager%3E++", &title);
        let response = request(&app, &new, &owner, Some(&invalid), true).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            text(response)
                .await
                .contains("name, job title, email, and phone")
        );
    }
    let valid = draft.replace("2026-02-30", "2026-10-08");
    let response = request(&app, &new, &owner, Some(&valid), true).await;
    assert_eq!(response.headers()["HX-Redirect"], base);
    let id: i64 = sqlx::query_scalar("SELECT id FROM events WHERE title='Round 3'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let person: (String, String, String, String, String, String) = sqlx::query_as(
        "SELECT first_name, last_name, job_title, email, phone, notes FROM people JOIN event_people ON event_people.person_id = people.id WHERE event_id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        person,
        (
            "John".into(),
            "Smith".into(),
            "<Hiring manager>".into(),
            "john@example.com".into(),
            "+1 555 1234".into(),
            "Manager".into()
        )
    );
    let timeline = text(request(&app, &base, &owner, None, false).await).await;
    assert!(timeline.contains("&lt;Hiring manager&gt;") && !timeline.contains("<Hiring manager>"));
    assert!(
        timeline.contains("class=\"person-avatar\" aria-label=\"Contact details for John Smith\"")
    );
    assert!(timeline.contains("<span aria-hidden=\"true\">JS</span>"));
    assert!(timeline.contains("href=\"tel:+1 555 1234\""));
    assert!(timeline.contains("href=\"mailto:john@example.com\""));
    let edit = format!("{base}/events/{id}/edit");
    let delete = format!("{base}/events/{id}/delete");
    let edit_page = text(request(&app, &edit, &owner, None, false).await).await;
    assert!(edit_page.contains("name=\"occurs_at\" value=\"2026-10-08T14:30\""));
    for path in [&edit, &delete] {
        assert_eq!(
            request(&app, path, &other, None, false).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &app,
                path,
                &other,
                Some(&format!("csrf={other_token}")),
                true
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    let timeline = text(request(&app, &base, &owner, None, false).await).await;
    assert!(
        timeline.find("Round 3").unwrap()
            < timeline
                .find(r#"<h3 class="h5 mt-1 text-break">Applied</h3>"#)
                .unwrap()
    );
    assert!(timeline.contains("john@example.com") && !timeline.contains("<script>draft"));
    let response = request(&app, &edit, &owner, Some(&format!("csrf={token}&title=Completed&occurs_at=2026-10-07T15%3A00&notes=Done&person_0_first_name=Jane")), false).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT first_name FROM people JOIN event_people ON event_people.person_id = people.id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(names, vec!["Jane"]);
    assert!(
        text(request(&app, &delete, &owner, None, false).await)
            .await
            .contains("Delete event?")
    );
    assert_eq!(
        request(&app, &delete, &owner, Some(&format!("csrf={token}")), false)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM event_people")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    request(&app, &new, &owner, Some(&valid), false).await;
    request(
        &app,
        &format!("{base}/delete"),
        &owner,
        Some(&format!("csrf={token}")),
        false,
    )
    .await;
    for table in ["events", "event_people", "people"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[tokio::test]
async fn event_people_failures_roll_back_and_return_safe_errors() {
    use jobtowel::models::{
        event::{Event, Input},
        person::Input as Person,
        role::Role,
    };
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "eventfailure", "eventfailure@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    request(
        &app,
        "/applied/new",
        &owner,
        Some(&format!("csrf={token}&role_title=Engineer&company=Example")),
        false,
    )
    .await;
    let user: i64 = sqlx::query_scalar("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let role = Role::list(&pool, user).await.unwrap().remove(0);
    let event = Event::list(&pool, user, role.id).await.unwrap().remove(0);
    sqlx::raw_sql("CREATE TRIGGER reject_people BEFORE INSERT ON event_people BEGIN SELECT RAISE(ABORT, 'private database failure'); END;").execute(&pool).await.unwrap();
    let response = request(
        &app,
        &format!("/applied/{}/events/{}/edit", role.id, event.id),
        &owner,
        Some(&format!(
            "csrf={token}&title=Changed&occurs_at=2026-10-07T15%3A00&person_0_first_name=John"
        )),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.headers()["HX-Retarget"], "#request-error");
    assert!(!text(response).await.contains("private database failure"));
    assert_eq!(
        Event::find(&pool, user, role.id, event.id)
            .await
            .unwrap()
            .unwrap()
            .title,
        "Applied"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM people")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    for (date, first_name, email) in [
        ("2026-02-29T12:00", "John", ""),
        ("2026-10-07T24:00", "John", ""),
        ("2026-10-07T12:00", "John", "bad"),
        ("2026-10-07T12:00", "", "john@example.com"),
    ] {
        assert!(
            Input {
                title: "Interview".into(),
                occurs_at: date.into(),
                people: vec![Person {
                    first_name: first_name.into(),
                    email: email.into(),
                    ..Person::default()
                }],
                ..Input::default()
            }
            .validate("UTC")
            .is_err()
        );
    }
}

#[tokio::test]
async fn event_title_choices_validate_save_and_preserve_custom_drafts() {
    use jobtowel::models::{
        event::TITLE_OPTIONS,
        role::{Input, Role},
    };
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "titleowner", "titles@example.com").await;
    let user_id = sqlx::query_scalar("SELECT id FROM users WHERE username = 'titleowner'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let role = Role::create(
        &pool,
        user_id,
        Input {
            role_title: "Engineer".into(),
            company: "Example".into(),
            ..Input::default()
        }
        .validate()
        .unwrap(),
    )
    .await
    .unwrap();
    let path = format!("/applied/{}/events/new", role.id);
    let html = text(request(&app, &path, &owner, None, false).await).await;
    let token = csrf(&html);
    assert!(html.contains("<select class=\"form-select\""));
    for title in TITLE_OPTIONS {
        assert!(html.contains(&format!("value=\"{title}\"")));
        let body = format!(
            "csrf={token}&title_choice={}&title=Unused&occurs_at=2026-10-07T15%3A00&removal_status=keep",
            title.replace(' ', "+")
        );
        assert_eq!(
            request(&app, &path, &owner, Some(&body), false)
                .await
                .status(),
            StatusCode::SEE_OTHER
        );
        let id: i64 = sqlx::query_scalar("SELECT id FROM events WHERE title = ?")
            .bind(title)
            .fetch_one(&pool)
            .await
            .unwrap();
        let edit = text(
            request(
                &app,
                &format!("/applied/{}/events/{id}/edit", role.id),
                &owner,
                None,
                false,
            )
            .await,
        )
        .await;
        assert!(edit.contains(&format!("value=\"{title}\" selected")));
    }
    for hx in [false, true] {
        for (choice, title) in [("custom", ""), ("unknown", "Draft"), ("", "")] {
            let body = format!(
                "csrf={token}&title_choice={choice}&title={title}&occurs_at=2026-10-07T15%3A00&notes=Keep+notes"
            );
            let response = request(&app, &path, &owner, Some(&body), hx).await;
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            assert!(text(response).await.contains("Keep notes"));
        }
        let body = format!(
            "csrf={token}&title_choice=custom&title=%3Cb%3EFollow+up%3C%2Fb%3E&occurs_at=bad&notes=Draft"
        );
        let response = request(&app, &path, &owner, Some(&body), hx).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html = text(response).await;
        assert!(html.contains("value=\"custom\" selected"));
        assert!(html.contains("value=\"&lt;b&gt;Follow up&lt;/b&gt;\""));
    }
    let body =
        format!("csrf={token}&title_choice=custom&title=Follow+up&occurs_at=2026-10-07T15%3A00");
    assert_eq!(
        request(&app, &path, &owner, Some(&body), false)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    let id: i64 = sqlx::query_scalar("SELECT id FROM events WHERE title = 'Follow up'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let edit = text(
        request(
            &app,
            &format!("/applied/{}/events/{id}/edit", role.id),
            &owner,
            None,
            false,
        )
        .await,
    )
    .await;
    assert!(edit.contains("value=\"custom\" selected"));
    assert!(edit.contains("value=\"Follow up\""));
}

#[tokio::test]
async fn dashboard_summary_counts_owned_roles_and_selects_events_by_date() {
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "statsowner", "statsowner@example.com").await;
    let other = signup(&app, "statsother", "statsother@example.com").await;
    let empty = text(request(&app, "/dashboard", &owner, None, false).await).await;
    assert!(empty.contains("id=\"application-count\" class=\"fs-2 fw-semibold mb-2\">0</p>"));
    assert!(empty.contains("No application events yet."));
    assert!(!empty.contains("upcoming-title"));
    let user: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'statsowner'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let outsider: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'statsother'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut ids = Vec::new();
    for (user, title, company) in [
        (user, "<b>Engineer</b>", "<script>Company</script>"),
        (user, "Designer", "Studio"),
        (outsider, "Private role", "Private company"),
    ] {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO roles (user_id, role_title, company) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(user)
        .bind(title)
        .bind(company)
        .fetch_one(&pool)
        .await
        .unwrap();
        ids.push(id);
    }
    sqlx::query("DELETE FROM events")
        .execute(&pool)
        .await
        .unwrap();
    for (role, title, modifier) in [
        (ids[0], "Old movement", "-3 days"),
        (ids[1], "Recent movement", "-1 day"),
        (ids[0], "<b>Latest movement</b>", "-1 day"),
        (ids[1], "Added later but older", "-2 days"),
        (ids[0], "<b>Upcoming interview</b>", "+2 days"),
        (ids[1], "Tied upcoming", "+2 days"),
        (ids[1], "Later appointment", "+5 days"),
        (ids[2], "Private movement", "-1 minute"),
        (ids[2], "Private appointment", "+1 minute"),
    ] {
        sqlx::query("INSERT INTO events (role_id, title, occurs_at) VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?))").bind(role).bind(title).bind(modifier).execute(&pool).await.unwrap();
    }
    let response = request(&app, "/dashboard", &owner, None, false).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = text(response).await;
    assert!(html.contains("id=\"application-count\" class=\"fs-2 fw-semibold mb-2\">2</p>"));
    for expected in [
        "&lt;b&gt;Engineer&lt;/b&gt;",
        "&lt;script&gt;Company&lt;/script&gt;",
        "&lt;b&gt;Latest movement&lt;/b&gt;",
        "&lt;b&gt;Upcoming interview&lt;/b&gt;",
    ] {
        assert!(html.contains(expected), "Missing {expected}");
    }
    assert!(html.contains(&format!("href=\"/applied/{}\"", ids[0])));
    for hidden in [
        "Private",
        "Tied upcoming",
        "Later appointment",
        "<b>Engineer</b>",
    ] {
        assert!(!html.contains(hidden), "Unexpected {hidden}");
    }
    let other_html = text(request(&app, "/dashboard", &other, None, false).await).await;
    assert!(!other_html.contains("Upcoming interview"));
    for modifier in ["+1 day", "+1 minute"] {
        sqlx::query("UPDATE events SET occurs_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) WHERE title = '<b>Upcoming interview</b>'").bind(modifier).execute(&pool).await.unwrap();
        let html = text(request(&app, "/dashboard", &owner, None, false).await).await;
        let date: String = sqlx::query_scalar(
            "SELECT occurs_at FROM events WHERE title = '<b>Upcoming interview</b>'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let expected = jobtowel::models::dates::display(&date, "UTC");
        assert!(html.contains(&format!(">{expected}</time>")));
    }
    sqlx::query("DELETE FROM events WHERE occurs_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .execute(&pool)
        .await
        .unwrap();
    let html = text(request(&app, "/dashboard", &owner, None, false).await).await;
    assert!(!html.contains("upcoming-title"));
    sqlx::query("DROP TABLE events")
        .execute(&pool)
        .await
        .unwrap();
    let response = request(&app, "/dashboard", &owner, None, false).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!text(response).await.contains("no such table"));
}

#[tokio::test]
async fn homepage_milestones_cover_fourteen_local_days_and_only_owned_events() {
    use chrono::{Duration, Utc};
    use jobtowel::models::{auth::User, dashboard::Summary, dates};

    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "milestoneowner", "milestoneowner@example.com").await;
    signup(&app, "milestoneother", "milestoneother@example.com").await;
    let user = User::by_username(&pool, "milestoneowner")
        .await
        .unwrap()
        .unwrap();
    let other = User::by_username(&pool, "milestoneother")
        .await
        .unwrap()
        .unwrap();
    let role: i64 = sqlx::query_scalar("INSERT INTO roles (user_id, role_title, company, active) VALUES (?, '<b>Engineer</b>', '<script>Company</script>', 0) RETURNING id")
        .bind(user.id).fetch_one(&pool).await.unwrap();
    let private_role: i64 = sqlx::query_scalar("INSERT INTO roles (user_id, role_title, company) VALUES (?, 'Private role', 'Private company') RETURNING id")
        .bind(other.id).fetch_one(&pool).await.unwrap();

    for timezone in ["Pacific/Kiritimati", "America/New_York", "Pacific/Honolulu"] {
        sqlx::query("UPDATE users SET timezone = ? WHERE id = ?")
            .bind(timezone)
            .bind(user.id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM events")
            .execute(&pool)
            .await
            .unwrap();
        let today = Utc::now()
            .with_timezone(&dates::zone(timezone))
            .date_naive();
        for (event_role, title, days, time) in [
            (role, "<b>Today event</b>", 0, "00:00"),
            (role, "Today later", 0, "23:59"),
            (role, "First included", 13, "00:00"),
            (role, "Too old", 14, "23:59"),
            (role, "Tomorrow", -1, "00:00"),
            (private_role, "Private event", 0, "00:00"),
        ] {
            let local = format!("{}T{time}", today - Duration::days(days));
            let utc = dates::to_utc(&local, timezone, "").unwrap();
            sqlx::query("INSERT INTO events (role_id, title, occurs_at) VALUES (?, ?, ?)")
                .bind(event_role)
                .bind(title)
                .bind(utc)
                .execute(&pool)
                .await
                .unwrap();
        }
        let summary = Summary::load(&pool, user.id).await.unwrap();
        assert_eq!(summary.milestone_days.len(), 14);
        for (index, day) in summary.milestone_days.iter().enumerate() {
            assert_eq!(day.date, today - Duration::days(index as i64));
        }
        assert_eq!(summary.milestone_days[0].events.len(), 2);
        assert_eq!(
            summary.milestone_days[0].events[0].event_title,
            "Today later"
        );
        assert_eq!(
            summary.milestone_days[13].events[0].event_title,
            "First included"
        );
        assert!(
            summary.milestone_days[1..13]
                .iter()
                .all(|day| day.events.is_empty())
        );
        let html = text(request(&app, "/", &owner, None, false).await).await;
        let milestones = html.split("id=\"milestones-title\"").nth(1).unwrap();
        assert_eq!(milestones.matches("<h3 ").count(), 14);
        for expected in [
            "Your milestones",
            "&lt;b&gt;Today event&lt;/b&gt;",
            "&lt;b&gt;Engineer&lt;/b&gt;",
            "&lt;script&gt;Company&lt;/script&gt;",
            "First included",
            "No events.",
        ] {
            assert!(
                milestones.contains(expected),
                "Missing {expected} in {timezone}"
            );
        }
        for hidden in ["Too old", "Tomorrow", "Private", "<b>Today event</b>"] {
            assert!(
                !milestones.contains(hidden),
                "Unexpected {hidden} in {timezone}"
            );
        }
        assert!(milestones.contains(&format!("href=\"/applied/{role}\"")));
    }
}

#[tokio::test]
async fn application_status_filter_removal_prompt_and_toggle() {
    use jobtowel::models::{dashboard::Summary, role::Role};
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "statusowner", "statusowner@example.com").await;
    let other = signup(&app, "statusother", "statusother@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    request(
        &app,
        "/applied/new",
        &owner,
        Some(&format!(
            "csrf={token}&role_title=Status+role&company=Example"
        )),
        false,
    )
    .await;
    let (role, user): (i64, i64) =
        sqlx::query_as("SELECT id, user_id FROM roles WHERE role_title = 'Status role'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let active = || Role::find(&pool, user, role);
    assert!(active().await.unwrap().unwrap().active);
    let new_event = format!("/applied/{role}/events/new");
    let event = format!(
        "csrf={token}&title_choice=Removed+from+consideration&occurs_at=2026-10-07T12%3A00&notes=Keep+my+draft&person_0_first_name=Alice"
    );
    for hx in [false, true] {
        let response = request(&app, &new_event, &owner, Some(&event), hx).await;
        assert_eq!(response.status(), StatusCode::OK);
        let html = text(response).await;
        assert!(
            html.contains("Mark this application inactive?")
                && html.contains("Keep my draft")
                && html.contains("Alice")
        );
        assert_eq!(html.contains("<!DOCTYPE"), !hx);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM events WHERE title = 'Removed from consideration'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert!(active().await.unwrap().unwrap().active);
    }
    let keep = request(
        &app,
        &new_event,
        &owner,
        Some(&format!("{event}&removal_status=keep")),
        false,
    )
    .await;
    assert_eq!(keep.status(), StatusCode::SEE_OTHER);
    assert!(active().await.unwrap().unwrap().active);
    // Both the event and status must roll back if a dependent write fails.
    sqlx::raw_sql("CREATE TRIGGER reject_status_people BEFORE INSERT ON event_people BEGIN SELECT RAISE(ABORT, 'private failure'); END;").execute(&pool).await.unwrap();
    let before = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            &new_event,
            &owner,
            Some(&format!("{event}&removal_status=inactive")),
            true
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert!(active().await.unwrap().unwrap().active);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
    sqlx::query("DROP TRIGGER reject_status_people")
        .execute(&pool)
        .await
        .unwrap();
    let removed = request(
        &app,
        &new_event,
        &owner,
        Some(&format!("{event}&removal_status=inactive")),
        true,
    )
    .await;
    assert_eq!(removed.headers()["HX-Redirect"], format!("/applied/{role}"));
    assert!(!active().await.unwrap().unwrap().active);
    assert_eq!(
        Summary::load(&pool, user).await.unwrap().application_count,
        0
    );
    assert!(
        !text(request(&app, "/applied", &owner, None, false).await)
            .await
            .contains(">Status role</td>")
    );
    for path in ["/applied?status=inactive", "/applied?status=all"] {
        let html = text(request(&app, path, &owner, None, true).await).await;
        assert!(
            html.contains(">Status role</td>")
                && html.contains(">Inactive</td>")
                && !html.contains("<!DOCTYPE")
        );
        assert!(
            !text(request(&app, path, &other, None, false).await)
                .await
                .contains(">Status role</td>")
        );
    }
    let edit = format!("/applied/{role}/edit");
    let draft = format!("csrf={token}&role_title=Status+role&company=Example&status_present=true");
    let invalid = request(
        &app,
        &edit,
        &owner,
        Some(&format!(
            "csrf={token}&role_title=&company=Example&status_present=true"
        )),
        true,
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!text(invalid).await.contains("value=\"true\" checked"));
    let other_token = csrf(&text(request(&app, "/applied/new", &other, None, false).await).await);
    assert_eq!(request(&app, &edit, &other, Some(&format!("csrf={other_token}&role_title=Status+role&company=Example&status_present=true&active=true")), false).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        request(
            &app,
            &edit,
            &owner,
            Some(&format!("{draft}&active=true")),
            true
        )
        .await
        .headers()["HX-Redirect"],
        "/applied"
    );
    assert!(active().await.unwrap().unwrap().active);
    assert_eq!(
        Summary::load(&pool, user).await.unwrap().application_count,
        1
    );
    request(&app, &edit, &owner, Some(&draft), false).await;
    assert!(!active().await.unwrap().unwrap().active);
    // An inactive status survives a later removal event when the current status is kept.
    request(
        &app,
        &new_event,
        &owner,
        Some(&format!("{event}&removal_status=keep")),
        false,
    )
    .await;
    assert!(!active().await.unwrap().unwrap().active);
}

#[tokio::test]
async fn timezone_signup_account_validation_and_initialization_are_persistent_and_protected() {
    use jobtowel::models::auth::User;
    let (app, pool, _dir) = app().await;
    let (guest, token) = entry(&app, "/signup").await;
    let draft = format!(
        "csrf={token}&username=zoned&name=Zoned+User&email=zoned%40example.com&password=long-password-123"
    );
    let response = request(
        &app,
        "/signup",
        &guest,
        Some(&format!("{draft}&timezone=invalid")),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let html = text(response).await;
    assert!(html.contains("Choose a valid timezone") && html.contains("value=\"invalid\""));
    assert!(html.contains("value=\"Zoned User\""));
    let response = request(
        &app,
        "/signup",
        &guest,
        Some(&format!("{draft}&timezone=America%2FNew_York")),
        true,
    )
    .await;
    assert_eq!(response.headers()["HX-Redirect"], "/dashboard");
    let mut owner = cookie(&response);
    let user = User::by_username(&pool, "zoned").await.unwrap().unwrap();
    assert_eq!(user.timezone(), "America/New_York");
    let html = text(request(&app, "/account", &owner, None, false).await).await;
    assert!(html.contains("value=\"America/New_York\" selected"));
    assert!(!html.contains("id=\"timezone-detection\""));
    let token = csrf(&html);
    for hx in [false, true] {
        let response = request(
            &app,
            "/account",
            &owner,
            Some(&format!(
                "csrf={token}&name=Keep+my+name&email=zoned%40example.com&timezone=%3Cscript%3E"
            )),
            hx,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html = text(response).await;
        assert_eq!(html.contains("<!DOCTYPE"), !hx);
        assert!(
            html.contains("value=\"&lt;script&gt;\" selected")
                && html.contains("value=\"Keep my name\"")
        );
        assert_eq!(
            User::by_id(&pool, user.id)
                .await
                .unwrap()
                .unwrap()
                .timezone(),
            "America/New_York"
        );
    }
    let response = request(
        &app,
        "/account",
        &owner,
        Some(&format!(
            "csrf={token}&name=Zoned+User&email=zoned%40example.com&timezone=Asia%2FKathmandu"
        )),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    owner = cookie(&response);
    assert_eq!(
        User::by_id(&pool, user.id)
            .await
            .unwrap()
            .unwrap()
            .timezone(),
        "Asia/Kathmandu"
    );
    let token = csrf(&text(request(&app, "/account", &owner, None, false).await).await);
    assert_eq!(
        request(
            &app,
            "/account/timezone",
            &owner,
            Some("csrf=wrong&timezone=UTC"),
            true
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "/account/timezone",
            "",
            Some(&format!("csrf={token}&timezone=UTC")),
            true
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "/account/timezone",
            &owner,
            Some(&format!("csrf={token}&timezone=Unknown")),
            true
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let response = request(
        &app,
        "/account/timezone",
        &owner,
        Some(&format!("csrf={token}&timezone=UTC")),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(response.headers()["X-Timezone-Initialized"], "false");
    assert_eq!(
        User::by_id(&pool, user.id)
            .await
            .unwrap()
            .unwrap()
            .timezone(),
        "Asia/Kathmandu"
    );
    // A legacy/unset profile detects once. Later browser locations cannot replace the choice.
    sqlx::query("UPDATE users SET timezone = NULL WHERE id = ?")
        .bind(user.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        text(request(&app, "/dashboard", &owner, None, false).await)
            .await
            .contains("id=\"timezone-detection\"")
    );
    let response = request(
        &app,
        "/account/timezone",
        &owner,
        Some(&format!("csrf={token}&timezone=Europe%2FLondon")),
        true,
    )
    .await;
    assert_eq!(response.headers()["X-Timezone-Initialized"], "true");
    assert_eq!(
        User::by_id(&pool, user.id)
            .await
            .unwrap()
            .unwrap()
            .timezone(),
        "Europe/London"
    );
}

#[tokio::test]
async fn event_dates_use_profile_timezone_for_defaults_writes_edits_and_all_displays() {
    use chrono::{Duration, SecondsFormat, TimeZone, Utc};
    use jobtowel::models::{
        auth::User, dashboard::Summary, dates, event::Input, role::AppliedRole,
    };
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "localdates", "localdates@example.com").await;
    let user = User::by_username(&pool, "localdates")
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE users SET timezone = 'America/New_York' WHERE id = ?")
        .bind(user.id)
        .execute(&pool)
        .await
        .unwrap();
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    request(
        &app,
        "/applied/new",
        &owner,
        Some(&format!("csrf={token}&role_title=Engineer&company=Example")),
        false,
    )
    .await;
    let role: i64 = sqlx::query_scalar("SELECT id FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    let base = format!("/applied/{role}");
    let new = format!("{base}/events/new");
    let before = Input::for_new("America/New_York").occurs_at;
    let html = text(request(&app, &new, &owner, None, false).await).await;
    let after = Input::for_new("America/New_York").occurs_at;
    let default = html
        .split("name=\"occurs_at\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(default >= before.as_str() && default <= after.as_str());
    assert!(html.contains(">Date and time</label>"));
    assert!(!html.contains("Date and time ("));
    for hx in [false, true] {
        let draft = format!("csrf={token}&title=Gap&occurs_at=2026-03-08T02%3A30&notes=Keep+this");
        let response = request(&app, &new, &owner, Some(&draft), hx).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html = text(response).await;
        assert!(
            html.contains("does not exist")
                && html.contains("value=\"2026-03-08T02:30\"")
                && html.contains("Keep this")
        );
        let response = request(
            &app,
            &new,
            &owner,
            Some(&format!(
                "csrf={token}&title=Fold&occurs_at=2026-11-01T01%3A30"
            )),
            hx,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html = text(response).await;
        assert!(html.contains("Which occurrence?") && html.contains("value=\"-05:00\""));
    }
    let response = request(
        &app,
        &new,
        &owner,
        Some(&format!(
            "csrf={token}&title=Fold&occurs_at=2026-11-01T01%3A30&utc_offset=-05%3A00"
        )),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let (id, stored): (i64, String) =
        sqlx::query_as("SELECT id, occurs_at FROM events WHERE title = 'Fold'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, "2026-11-01T06:30:00.000Z");
    let edit = format!("{base}/events/{id}/edit");
    let html = text(request(&app, &edit, &owner, None, false).await).await;
    assert!(
        html.contains("value=\"2026-11-01T01:30\"") && html.contains("value=\"-05:00\" selected")
    );
    request(
        &app,
        &edit,
        &owner,
        Some(&format!(
            "csrf={token}&title=Fold&occurs_at=2026-11-01T01%3A30&utc_offset=-05%3A00"
        )),
        true,
    )
    .await;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT occurs_at FROM events WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        stored
    );
    sqlx::query("UPDATE users SET timezone = 'Asia/Kathmandu' WHERE id = ?")
        .bind(user.id)
        .execute(&pool)
        .await
        .unwrap();
    let html = text(request(&app, &edit, &owner, None, false).await).await;
    assert!(html.contains("value=\"2026-11-01T12:15\""));
    let timeline = text(request(&app, &base, &owner, None, false).await).await;
    assert!(timeline.contains("November 01, 2026 at 12:15 PM"));
    let response = request(
        &app,
        &edit,
        &owner,
        Some(&format!(
            "csrf={token}&title=Fold&occurs_at=2026-11-01T01%3A30&timezone=America%2FNew_York"
        )),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        text(response)
            .await
            .contains("Your account date settings changed")
    );
    // Use tomorrow's local calendar date for both application and dashboard relative dates.
    let tomorrow = Utc::now()
        .with_timezone(&dates::zone("Asia/Kathmandu"))
        .date_naive()
        + Duration::days(1);
    let future = dates::zone("Asia/Kathmandu")
        .from_local_datetime(&tomorrow.and_hms_opt(0, 30, 0).unwrap())
        .single()
        .unwrap()
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    sqlx::query("UPDATE events SET occurs_at = ? WHERE id = ?")
        .bind(&future)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let application = AppliedRole::list(&pool, user.id).await.unwrap().remove(0);
    assert_eq!(application.last_event_days_ago, Some(-1));
    assert_eq!(
        Summary::load(&pool, user.id)
            .await
            .unwrap()
            .upcoming_event
            .unwrap()
            .days_until,
        1
    );
    let listing = text(request(&app, "/applied", &owner, None, false).await).await;
    assert!(listing.contains(&dates::display(&future, "Asia/Kathmandu")));
    let fragment = text(request(&app, "/applied", &owner, None, true).await).await;
    assert!(fragment.contains(&dates::display(&future, "Asia/Kathmandu")));
    let dashboard = text(request(&app, "/dashboard", &owner, None, false).await).await;
    assert!(dashboard.contains(&dates::display(&future, "Asia/Kathmandu")));
}

#[tokio::test]
async fn application_people_are_reused_scoped_and_deleted_after_last_link() {
    use jobtowel::models::{auth::User, event::Event, person::Person, role::Role};
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "peopleowner", "peopleowner@example.com").await;
    let other = signup(&app, "peopleother", "peopleother@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    let other_token = csrf(&text(request(&app, "/applied/new", &other, None, false).await).await);
    for (cookie, csrf) in [(&owner, &token), (&owner, &token), (&other, &other_token)] {
        request(
            &app,
            "/applied/new",
            cookie,
            Some(&format!("csrf={csrf}&role_title=Engineer&company=Example")),
            false,
        )
        .await;
    }
    let user = User::by_username(&pool, "peopleowner")
        .await
        .unwrap()
        .unwrap();
    let roles = Role::list(&pool, user.id).await.unwrap();
    let role = roles[0].id;
    let path = format!("/applied/{role}/events/new");
    let draft = format!(
        "csrf={token}&title=First&occurs_at=2026-10-08T12%3A00&person_0_first_name=%3CJo%3E&person_0_email=jo%40example.com"
    );
    assert_eq!(
        request(&app, &path, &owner, Some(&draft), false)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    let person = Person::list(&pool, user.id, role).await.unwrap().remove(0);
    assert_eq!(person.role_id, role);
    let selection = format!(
        "csrf={token}&title=Second&occurs_at=2026-10-09T12%3A00&person_0_id={}&person_0_first_name=Ignored",
        person.id
    );
    let html = text(
        request(
            &app,
            &path,
            &owner,
            Some(&format!("{selection}&people_action=add")),
            true,
        )
        .await,
    )
    .await;
    assert!(
        html.contains("&lt;Jo&gt;") && html.contains(&format!("value=\"{}\" selected", person.id))
    );
    assert_eq!(
        request(&app, &path, &owner, Some(&selection), true)
            .await
            .headers()["HX-Redirect"],
        format!("/applied/{role}")
    );
    assert_eq!(Person::list(&pool, user.id, role).await.unwrap().len(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event_people WHERE person_id = ?")
            .bind(person.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    let html = text(
        request(
            &app,
            &path,
            &owner,
            Some(&format!("{selection}&person_1_id={}", person.id)),
            true,
        )
        .await,
    )
    .await;
    assert!(html.contains("Each person can only be added once"));
    for bad in [person.id.to_string(), "99999".into(), "invalid".into()] {
        let response = request(
            &app,
            &format!("/applied/{}/events/new", roles[1].id),
            &owner,
            Some(&format!(
                "csrf={token}&title=Bad&occurs_at=2026-10-08T12%3A00&notes=Keep&person_0_id={bad}"
            )),
            true,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html = text(response).await;
        assert!(
            html.contains("Choose an existing person from this application")
                && html.contains("Keep")
        );
    }
    let foreign_role: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE user_id != ?")
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = request(
        &app,
        &format!("/applied/{foreign_role}/events/new"),
        &other,
        Some(&selection.replace(&token, &other_token)),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        Person::list(&pool, user.id, foreign_role)
            .await
            .unwrap()
            .is_empty()
    );
    let first = Event::list(&pool, user.id, role)
        .await
        .unwrap()
        .into_iter()
        .find(|event| event.title == "First")
        .unwrap();
    let first_edit = format!("/applied/{role}/events/{}/edit", first.id);
    let response = request(
        &app,
        &first_edit,
        &owner,
        Some(&format!(
            "csrf={token}&title=First&occurs_at=2026-10-08T12%3A00"
        )),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(first.people(&pool).await.unwrap().is_empty());
    assert_eq!(Person::list(&pool, user.id, role).await.unwrap().len(), 1);
    let events = Event::list(&pool, user.id, role).await.unwrap();
    for event in events {
        if event.title == "First" || event.title == "Second" {
            let edit = format!("/applied/{role}/events/{}/edit", event.id);
            let html = text(request(&app, &edit, &owner, None, false).await).await;
            if event.title == "Second" {
                assert!(html.contains(&format!("value=\"{}\" selected", person.id)));
            }
            request(
                &app,
                &format!("/applied/{role}/events/{}/delete", event.id),
                &owner,
                Some(&format!("csrf={token}")),
                false,
            )
            .await;
        }
    }
    assert!(Person::list(&pool, user.id, role).await.unwrap().is_empty());
    let foreign_event: i64 = sqlx::query_scalar("SELECT id FROM events WHERE role_id = ?")
        .bind(roles[1].id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("INSERT INTO event_people (event_id, person_id, role_id) VALUES (?, ?, ?)")
            .bind(foreign_event)
            .bind(person.id)
            .bind(roles[1].id)
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn last_person_link_cleanup_is_atomic_and_application_scoped() {
    use jobtowel::models::{event::Event, person::Person};
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "cleanup", "cleanup@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    for title in ["One", "Two"] {
        request(
            &app,
            "/applied/new",
            &owner,
            Some(&format!("csrf={token}&role_title={title}&company=Example")),
            false,
        )
        .await;
    }
    let user: i64 = sqlx::query_scalar("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let roles: Vec<i64> = sqlx::query_scalar("SELECT id FROM roles ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    for role in &roles {
        let response = request(
            &app,
            &format!("/applied/{role}/events/new"),
            &owner,
            Some(&format!(
                "csrf={token}&title=Interview&occurs_at=2026-10-08T12%3A00&person_0_first_name=Jo"
            )),
            false,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }
    let person = Person::list(&pool, user, roles[0]).await.unwrap().remove(0);
    let event = Event::list(&pool, user, roles[0])
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.title == "Interview")
        .unwrap();
    let edit = format!("/applied/{}/events/{}/edit", roles[0], event.id);
    let delete = format!("/applied/{}/events/{}/delete", roles[0], event.id);
    let draft = format!("csrf={token}&title=Interview&occurs_at=2026-10-08T12%3A00");
    let response = request(
        &app,
        &edit,
        &owner,
        Some(&format!("{draft}&person_0_id={}", person.id)),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(event.people(&pool).await.unwrap()[0].id, person.id);
    assert!(
        !Event::delete(&pool, user + 1, roles[0], event.id)
            .await
            .unwrap()
    );
    sqlx::raw_sql("CREATE TRIGGER reject_person_cleanup BEFORE DELETE ON people BEGIN SELECT RAISE(ABORT, 'private cleanup failure'); END;").execute(&pool).await.unwrap();
    for (path, body) in [(&edit, draft.clone()), (&delete, format!("csrf={token}"))] {
        let response = request(&app, path, &owner, Some(&body), true).await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.headers()["HX-Retarget"], "#request-error");
        assert!(!text(response).await.contains("private cleanup failure"));
        assert!(
            Event::find(&pool, user, roles[0], event.id)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(event.people(&pool).await.unwrap()[0].id, person.id);
    }
    sqlx::query("DROP TRIGGER reject_person_cleanup")
        .execute(&pool)
        .await
        .unwrap();
    // Removing a draft row alone must not delete anything before the event is saved.
    let response = request(
        &app,
        &edit,
        &owner,
        Some(&format!(
            "{draft}&person_0_id={}&people_action=remove%3A0",
            person.id
        )),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(event.people(&pool).await.unwrap()[0].id, person.id);
    let response = request(&app, &edit, &owner, Some(&draft), true).await;
    assert_eq!(
        response.headers()["HX-Redirect"],
        format!("/applied/{}", roles[0])
    );
    assert!(event.people(&pool).await.unwrap().is_empty());
    assert!(
        Person::list(&pool, user, roles[0])
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(Person::list(&pool, user, roles[1]).await.unwrap().len(), 1);
}

#[tokio::test]
async fn person_updates_preserve_links_validate_drafts_and_enforce_ownership() {
    use jobtowel::models::person::Person;
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "personeditor", "editor@example.com").await;
    let other = signup(&app, "personoutsider", "outsider@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    let response = request(
        &app,
        "/applied/new",
        &owner,
        Some(&format!("csrf={token}&role_title=Engineer&company=Example")),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let (role, user): (i64, i64) = sqlx::query_as("SELECT id, user_id FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    for title in ["First", "Second"] {
        let person = if title == "First" {
            "person_0_first_name=Jo&person_0_last_name=Lee&person_0_job_title=Manager&person_0_email=jo%40example.com&person_0_phone=123&person_0_notes=Original".to_owned()
        } else {
            format!(
                "person_0_id={}",
                Person::list(&pool, user, role).await.unwrap()[0].id
            )
        };
        assert_eq!(
            request(
                &app,
                &format!("/applied/{role}/events/new"),
                &owner,
                Some(&format!(
                    "csrf={token}&title={title}&occurs_at=2026-10-08T12%3A00&{person}"
                )),
                false
            )
            .await
            .status(),
            StatusCode::SEE_OTHER
        );
    }
    let id = Person::list(&pool, user, role).await.unwrap()[0].id;
    let path = format!("/applied/{role}/people/{id}/edit");
    let timeline =
        text(request(&app, &format!("/applied/{role}"), &owner, None, false).await).await;
    assert_eq!(timeline.matches(&format!("href=\"{path}\"")).count(), 2);
    assert!(timeline.contains("aria-label=\"Update Jo Lee\""));
    let page = text(request(&app, &path, &owner, None, false).await).await;
    for value in ["Jo", "Lee", "Manager", "jo@example.com", "123"] {
        assert!(page.contains(&format!("value=\"{value}\"")));
    }
    assert!(page.contains(">Original</textarea>"));
    assert!(page.contains("hx-target=\"#person-form\""));
    assert_eq!(
        request(&app, &path, "", None, false).await.headers()["location"],
        "/login"
    );
    let other_token = csrf(&text(request(&app, "/account", &other, None, false).await).await);
    for (target, cookie, csrf) in [
        (path.clone(), &other, other_token.as_str()),
        (
            format!("/applied/99999/people/{id}/edit"),
            &owner,
            token.as_str(),
        ),
        (
            format!("/applied/{role}/people/99999/edit"),
            &owner,
            token.as_str(),
        ),
    ] {
        assert_eq!(
            request(&app, &target, cookie, None, false).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &app,
                &target,
                cookie,
                Some(&format!("csrf={csrf}&first_name=Changed")),
                false
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        request(
            &app,
            &path,
            &owner,
            Some("csrf=wrong&first_name=Changed"),
            false
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    for hx in [false, true] {
        for invalid in [
            "first_name=&email=valid%40example.com",
            "first_name=Draft&email=bad",
            "first_name=Draft&job_title=%00",
        ] {
            let response = request(&app, &path, &owner, Some(&format!("csrf={token}&{invalid}&last_name=%3CLee%3E&notes=%3Cscript%3Edraft%3C%2Fscript%3E&id={id}")), hx).await;
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let html = text(response).await;
            assert!(
                html.contains("id=\"person-validation-error\"")
                    && html.contains("value=\"&lt;Lee&gt;\"")
            );
            assert!(
                html.contains("&lt;script&gt;draft&lt;/script&gt;")
                    && !html.contains("<script>draft")
            );
            assert_eq!(html.contains("<!DOCTYPE html>"), !hx);
        }
    }
    assert_eq!(
        Person::find(&pool, user, role, id)
            .await
            .unwrap()
            .unwrap()
            .first_name,
        "Jo"
    );
    sqlx::raw_sql("CREATE TRIGGER reject_person_update BEFORE UPDATE ON people BEGIN SELECT RAISE(ABORT, 'private update failure'); END;").execute(&pool).await.unwrap();
    let response = request(
        &app,
        &path,
        &owner,
        Some(&format!("csrf={token}&first_name=Changed")),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.headers()["HX-Retarget"], "#request-error");
    assert!(!text(response).await.contains("private update failure"));
    sqlx::query("DROP TRIGGER reject_person_update")
        .execute(&pool)
        .await
        .unwrap();
    for hx in [true, false] {
        let response = request(&app, &path, &owner, Some(&format!("csrf={token}&first_name=++%3CJane%3E++&last_name=Smith&job_title=Director&email=jane%40example.com&phone=%2B1+555&notes=Updated")), hx).await;
        if hx {
            assert_eq!(
                response.headers()["HX-Redirect"],
                format!("/applied/{role}")
            );
        } else {
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(response.headers()["location"], format!("/applied/{role}"));
        }
    }
    let person = Person::find(&pool, user, role, id).await.unwrap().unwrap();
    assert_eq!(
        (
            person.first_name.as_str(),
            person.last_name.as_str(),
            person.job_title.as_str(),
            person.email.as_str(),
            person.phone.as_str(),
            person.notes.as_str()
        ),
        (
            "<Jane>",
            "Smith",
            "Director",
            "jane@example.com",
            "+1 555",
            "Updated"
        )
    );
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM event_people WHERE person_id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(links, 2);
    let timeline =
        text(request(&app, &format!("/applied/{role}"), &owner, None, false).await).await;
    assert_eq!(timeline.matches("mailto:jane@example.com").count(), 2);
    assert_eq!(timeline.matches("&lt;Jane&gt;</span>").count(), 2);
    assert_eq!(
        request(
            &app,
            &path,
            &owner,
            Some(&format!("csrf={token}&first_name=Jane")),
            false
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    let person = Person::find(&pool, user, role, id).await.unwrap().unwrap();
    assert!(
        person.last_name.is_empty()
            && person.job_title.is_empty()
            && person.email.is_empty()
            && person.phone.is_empty()
            && person.notes.is_empty()
    );
}

#[tokio::test]
async fn markdown_notes_render_safely_everywhere_and_edit_as_source() {
    let (app, pool, _dir) = app().await;
    let owner = signup(&app, "markdown", "markdown@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    let source = "# Small heading\n\n**Important** and *emphasis*\n\n- One\n- Two\n\n[Site](https://example.com)\n\n<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n[Unsafe](javascript:alert(1))\n\n<div id=auth-form hx-post=/logout style=position:fixed>Plain</div>";
    let response = request(&app, "/applied/new", &owner, Some(&format!("csrf={token}&role_title=Markdown&company=Example&role_description=%23+Job+notes%0A%0A**Saved**")), false).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let role: i64 = sqlx::query_scalar("SELECT id FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    let stored: String = sqlx::query_scalar("SELECT role_description FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, "# Job notes\n\n**Saved**");
    let event: i64 = sqlx::query_scalar("SELECT id FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET notes = ? WHERE id = ?")
        .bind(source)
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();
    let person: i64 = sqlx::query_scalar(
        "INSERT INTO people (role_id, first_name, notes) VALUES (?, 'Jo', ?) RETURNING id",
    )
    .bind(role)
    .bind(source)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO event_people (event_id, person_id, role_id) VALUES (?, ?, ?)")
        .bind(event)
        .bind(person)
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    let html = text(request(&app, &format!("/applied/{role}"), &owner, None, false).await).await;
    assert!(html.contains("<h1>Job notes</h1>") && html.contains("<strong>Saved</strong>"));
    assert_eq!(html.matches("<h1>Small heading</h1>").count(), 2);
    assert_eq!(html.matches("<strong>Important</strong>").count(), 2);
    assert!(
        html.contains("<em>emphasis</em>")
            && html.contains("<ul>")
            && html.contains("href=\"https://example.com\"")
    );
    assert!(
        !html.contains("alert(1)")
            && !html.contains("javascript:")
            && !html.contains("onerror")
            && !html.contains("style=\"position:fixed\"")
            && !html.contains("hx-post=\"/logout\"")
    );
    for (path, name, escaped_source) in [
        (
            format!("/applied/{role}/edit"),
            "role_description",
            "# Job notes",
        ),
        (
            format!("/applied/{role}/events/{event}/edit"),
            "notes",
            "&lt;script&gt;alert(1)&lt;/script&gt;",
        ),
        (
            format!("/applied/{role}/people/{person}/edit"),
            "notes",
            "&lt;script&gt;alert(1)&lt;/script&gt;",
        ),
    ] {
        let html = text(request(&app, &path, &owner, None, false).await).await;
        assert!(html.contains("data-markdown-editor") && html.contains("data-markdown-preview"));
        assert!(html.contains(&format!("name=\"{name}\"")) && html.contains(escaped_source));
    }
}

#[tokio::test]
async fn markdown_preview_requires_session_csrf_and_bounded_safe_content() {
    let (app, _pool, _dir) = app().await;
    let owner = signup(&app, "preview", "preview@example.com").await;
    let token = csrf(&text(request(&app, "/applied/new", &owner, None, false).await).await);
    let response = request(
        &app,
        "/notes/preview",
        "",
        Some("csrf=bad&notes=Draft"),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["location"], "/login");
    assert_eq!(
        request(
            &app,
            "/notes/preview",
            &owner,
            Some("csrf=bad&notes=Draft"),
            true
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = request(&app, "/notes/preview", &owner, Some(&format!("csrf={token}&notes=%23+Preview%0A%0A**Bold**%0A%0A%3Cscript%3Ealert(1)%3C%2Fscript%3E%0A%0A%5BUnsafe%5D(javascript%3Aalert(1))")), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = text(response).await;
    assert!(html.contains("<h1>Preview</h1>") && html.contains("<strong>Bold</strong>"));
    assert!(!html.contains("<script") && !html.contains("javascript:"));
    let response = request(
        &app,
        "/notes/preview",
        &owner,
        Some(&format!("csrf={token}&notes={}", "a".repeat(10001))),
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(text(response).await.contains("10,000 characters"));
}
