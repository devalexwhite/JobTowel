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
            && html.contains("No applications recorded")
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
    assert!(listing.contains(">Last Event</th>"));
    assert!(listing.contains("<td class=\"text-break\">Applied<time"));
    assert!(listing.contains(">Today</time>"));
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
    for (modifier, expected) in [
        ("0 days", "Today"),
        ("-1 day", "Yesterday"),
        ("-2 days", "2 days ago"),
        ("-7 days", "A week ago"),
        ("-30 days", "A month ago"),
        ("+2 days", "In 2 days"),
    ] {
        sqlx::query("UPDATE events SET occurs_at = strftime('%Y-%m-%dT00:00:00.000Z', 'now', ?) WHERE role_id = ?")
            .bind(modifier).bind(role.id).execute(&pool).await.unwrap();
        let listing = text(request(&app, "/applied", &owner, None, false).await).await;
        assert!(listing.contains(&format!(">{expected}</time>")));
        let latest = AppliedRole::list(&pool, user_id).await.unwrap().remove(0);
        assert!(listing.contains(&format!("datetime=\"{}\"", latest.last_event_at.unwrap())));
    }
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
    let base = format!("/applied/{role}");
    let new = format!("{base}/events/new");
    assert_eq!(
        request(&app, &new, "", None, false).await.headers()["location"],
        "/login"
    );
    assert_eq!(
        request(&app, &base, &other, None, false).await.status(),
        StatusCode::NOT_FOUND
    );
    let draft = format!(
        "csrf={token}&title=Round+3&occurs_at=2026-02-30T14%3A30&notes=%3Cscript%3Edraft%3C%2Fscript%3E&person_0_first_name=John&person_0_last_name=Smith&person_0_email=john%40example.com&person_0_phone=%2B1+555+1234&person_0_notes=Manager"
    );
    for hx in [false, true] {
        let response = request(&app, &new, &owner, Some(&draft), hx).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html = text(response).await;
        assert_eq!(html.contains("<!DOCTYPE"), !hx);
        assert!(
            html.contains("id=\"event-form\"")
                && html.contains("&lt;script&gt;draft&lt;/script&gt;")
                && html.contains("value=\"John\"")
                && html.contains("value=\"Smith\"")
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
    let valid = draft.replace("2026-02-30", "2026-10-08");
    let response = request(&app, &new, &owner, Some(&valid), true).await;
    assert_eq!(response.headers()["HX-Redirect"], base);
    let id: i64 = sqlx::query_scalar("SELECT id FROM events WHERE title='Round 3'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let person: (String, String, String, String, String) = sqlx::query_as(
        "SELECT first_name, last_name, email, phone, notes FROM event_people WHERE event_id = ?",
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
            "john@example.com".into(),
            "+1 555 1234".into(),
            "Manager".into()
        )
    );
    let edit = format!("{base}/events/{id}/edit");
    let delete = format!("{base}/events/{id}/delete");
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
    let names: Vec<String> = sqlx::query_scalar("SELECT first_name FROM event_people")
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
    for table in ["events", "event_people"] {
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
        event::{Event, Input, Person},
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
            .validate()
            .is_err()
        );
    }
}
