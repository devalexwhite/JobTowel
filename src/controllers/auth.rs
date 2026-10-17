use crate::{
    AppState,
    error::AppError,
    models::auth::{self, Input, Session, User},
    views,
};
use axum::{
    Form,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;

fn cookie_token(headers: &HeaderMap) -> &str {
    headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(';')
        .filter_map(|s| s.trim().split_once('='))
        .find(|(name, _)| *name == "jobtowel_session")
        .map(|(_, value)| value)
        .unwrap_or("")
}
fn cookie(state: &AppState, token: &str, age: u32) -> String {
    let secure = if state.session_cookie_secure {
        "; Secure"
    } else {
        ""
    };
    format!("jobtowel_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={age}{secure}")
}
pub(super) fn htmx(headers: &HeaderMap) -> bool {
    headers.get("HX-Request").is_some_and(|v| v == "true")
}
pub(super) fn redirect(headers: &HeaderMap, path: &str) -> Response {
    if htmx(headers) {
        (StatusCode::OK, [("HX-Redirect", path)]).into_response()
    } else {
        Redirect::to(path).into_response()
    }
}
async fn session(state: &AppState, headers: &HeaderMap) -> Result<Option<Session>, AppError> {
    Ok(Session::find(&state.pool, cookie_token(headers)).await?)
}
pub(super) async fn signed_in(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<(Session, User)>, AppError> {
    if let Some(session) = session(state, headers).await?
        && let Some(id) = session.user_id
        && let Some(user) = User::by_id(&state.pool, id).await?
    {
        return Ok(Some((session, user)));
    }
    Ok(None)
}
pub(super) fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        "Your form expired. Reload the page and try again.",
    )
        .into_response()
}
fn invalid(
    headers: &HeaderMap,
    kind: &str,
    input: &Input,
    session: &Session,
    user: Option<&User>,
    message: &str,
) -> Response {
    let form = views::auth::form(kind, input, &session.csrf, Some(message));
    let view = if htmx(headers) {
        form
    } else {
        views::auth::page(kind, form, user, &session.csrf, false)
    };
    (StatusCode::UNPROCESSABLE_ENTITY, view).into_response()
}
async fn start(
    state: &AppState,
    headers: &HeaderMap,
    user_id: i64,
    path: &str,
) -> Result<Response, AppError> {
    let (token, _) = Session::create(&state.pool, Some(user_id)).await?;
    Session::delete(&state.pool, cookie_token(headers)).await?;
    let mut response = redirect(headers, path);
    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie(state, &token, 604800)
            .parse()
            .map_err(|_| AppError::Authentication)?,
    );
    Ok(response)
}
async fn entry(state: AppState, headers: HeaderMap, kind: &str) -> Result<Response, AppError> {
    if signed_in(&state, &headers).await?.is_some() {
        return Ok(redirect(&headers, "/dashboard"));
    }
    let (token, session) = Session::create(&state.pool, None).await?;
    Session::delete(&state.pool, cookie_token(&headers)).await?;
    let form = views::auth::form(kind, &Input::default(), &session.csrf, None);
    let mut response = views::auth::page(kind, form, None, &session.csrf, false).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie(&state, &token, 3600)
            .parse()
            .map_err(|_| AppError::Authentication)?,
    );
    Ok(response)
}
pub async fn signup_page(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    entry(state, headers, "signup").await
}
pub async fn login_page(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    entry(state, headers, "login").await
}
pub async fn signup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    let Some(session) = session(&state, &headers).await? else {
        return Ok(forbidden());
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    let profile = match input.signup() {
        Ok(p) => p,
        Err(e) => return Ok(invalid(&headers, "signup", &input, &session, None, e)),
    };
    let hash = auth::hash_password(input.password.clone()).await?;
    match User::create(&state.pool, &input.username, profile, &hash).await {
        Ok(user) => start(&state, &headers, user.id, "/dashboard").await,
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(invalid(
            &headers,
            "signup",
            &input,
            &session,
            None,
            "That username or email is already in use. Try another, or log in.",
        )),
        Err(e) => Err(e.into()),
    }
}
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    let Some(session) = session(&state, &headers).await? else {
        return Ok(forbidden());
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    let user = User::by_username(&state.pool, &input.username).await?;
    // Perform the same costly verification for unknown usernames.
    let hash = match &user {
        Some(u) => u.password_hash.clone(),
        None => auth::DUMMY_PASSWORD_HASH.to_owned(),
    };
    let valid = auth::verify(input.password.clone(), hash).await?;
    if let Some(user) = user.filter(|_| valid) {
        return start(&state, &headers, user.id, "/dashboard").await;
    }
    Ok(invalid(
        &headers,
        "login",
        &input,
        &session,
        None,
        "The username or password is incorrect.",
    ))
}
pub async fn dashboard(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    let summary = crate::models::dashboard::Summary::load(&state.pool, user.id).await?;
    Ok(views::dashboard::page(&user, &session.csrf, &summary).into_response())
}
#[derive(Deserialize)]
pub struct AccountQuery {
    saved: Option<String>,
}
pub async fn account_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AccountQuery>,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    let input = Input {
        name: user.name.clone(),
        email: user.email.clone(),
        timezone: Some(user.timezone().to_owned()),
        theme: Some(user.theme.clone()),
        ..Input::default()
    };
    Ok(views::auth::page(
        "account",
        views::auth::form("account", &input, &session.csrf, None),
        Some(&user),
        &session.csrf,
        query.saved.as_deref() == Some("1"),
    )
    .into_response())
}
pub async fn account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    let fail = |message| invalid(&headers, "account", &input, &session, Some(&user), message);
    let profile = match input.profile() {
        Ok(p) => p,
        Err(e) => return Ok(fail(e)),
    };
    if !input.password.is_empty()
        && let Err(e) = auth::validate_password(&input.password)
    {
        return Ok(fail(e));
    }
    if !input.password.is_empty()
        && !auth::verify(input.current_password.clone(), user.password_hash.clone()).await?
    {
        return Ok(fail("Enter your current password to change your password."));
    }
    let hash = if input.password.is_empty() {
        None
    } else {
        Some(auth::hash_password(input.password.clone()).await?)
    };
    match user.update(&state.pool, profile, hash).await {
        Ok(()) => start(&state, &headers, user.id, "/account?saved=1").await,
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Ok(fail("That email address is already in use."))
        }
        Err(e) => Err(e.into()),
    }
}
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    let Some(session) = session(&state, &headers).await? else {
        return Ok(redirect(&headers, "/login"));
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    Session::delete(&state.pool, cookie_token(&headers)).await?;
    let mut response = redirect(&headers, "/login");
    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie(&state, "", 0)
            .parse()
            .map_err(|_| AppError::Authentication)?,
    );
    Ok(response)
}

pub async fn initialize_timezone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<Input>,
) -> Result<Response, AppError> {
    let Some((session, user)) = signed_in(&state, &headers).await? else {
        return Ok(forbidden());
    };
    if !session.accepts(&input.csrf) {
        return Ok(forbidden());
    }
    let Some(timezone) = input
        .timezone
        .and_then(|zone| zone.parse::<chrono_tz::Tz>().ok())
    else {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, "Choose a valid timezone.").into_response());
    };
    let changed = user.initialize_timezone(&state.pool, timezone).await?;
    if !htmx(&headers) {
        return Ok(redirect(&headers, "/account"));
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(
            "X-Timezone-Initialized",
            if changed { "true" } else { "false" },
        )],
    )
        .into_response())
}
