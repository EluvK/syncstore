use std::sync::Arc;

use salvo::{
    Depot, Request, Response, Router, Scribe, Writer,
    oapi::{RouterExt, ToResponse, ToSchema, endpoint, extract::JsonBody},
    writing::Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ServiceError, ServiceResult},
    store::Store,
    utils::jwt::{generate_jwt_token, generate_refresh_token, verify_refresh_token},
};

static COOKIE_HTTPS_ONLY: bool = false; // TODO: set to true in production

pub fn create_router() -> Router {
    Router::new()
}

pub fn create_non_auth_router() -> Router {
    Router::new()
        .push(Router::with_path("name-login").post(login))
        .push(Router::with_path("refresh").post(refresh))
        .oapi_tag("auth")
}

/// Login with username and password
///
/// Authenticates the user and returns an access token and a refresh token.
#[endpoint(
    status_codes(200, 401),
    request_body(content = NameLoginRequest, description = "Login by username and password"),
    responses(
        (status_code = 200, description = "Login successful", body = LoginResponse),
        (status_code = 401, description = "Unauthorized")
    )
)]
async fn login(
    req: JsonBody<NameLoginRequest>,
    depot: &mut Depot,
    resp: &mut Response,
) -> ServiceResult<LoginResponse> {
    let user_manager = depot.obtain::<Arc<Store>>()?.user_manager.clone();
    let Some(user_id) = user_manager.validate_user(&req.username, &req.password)? else {
        return Err(ServiceError::Unauthorized("Invalid username or password".to_string()));
    };
    let access_token = generate_jwt_token(user_id.clone())?;
    let refresh_token = generate_refresh_token(user_id.clone())?;

    resp.add_cookie(
        salvo::http::cookie::CookieBuilder::new("refresh_token", refresh_token.clone())
            .max_age(salvo::http::cookie::time::Duration::days(7))
            .same_site(salvo::http::cookie::SameSite::Lax)
            .http_only(true)
            .secure(COOKIE_HTTPS_ONLY)
            .build(),
    );

    Ok(LoginResponse {
        access_token,
        refresh_token,
        user_id,
    })
}

/// Refresh the access token using the refresh token
///
/// Returns a new access token and a new refresh token.
#[endpoint(
    status_codes(200, 401),
    responses(
        (status_code = 200, description = "Token refreshed successfully", body = LoginResponse),
        (status_code = 401, description = "Unauthorized")
    )
)]
async fn refresh(req: &mut Request, resp: &mut Response) -> ServiceResult<LoginResponse> {
    let refresh_token = req
        .cookies()
        .get("refresh_token")
        .ok_or_else(|| ServiceError::Unauthorized("No refresh token found".to_string()))?
        .value();
    let user_id = verify_refresh_token(refresh_token)?.sub;
    let access_token = generate_jwt_token(user_id.clone())?;
    let refresh_token = generate_refresh_token(user_id.clone())?;
    resp.add_cookie(
        salvo::http::cookie::CookieBuilder::new("refresh_token", refresh_token.clone())
            .max_age(salvo::http::cookie::time::Duration::days(7))
            .same_site(salvo::http::cookie::SameSite::Lax)
            .http_only(true)
            .secure(COOKIE_HTTPS_ONLY)
            .build(),
    );

    Ok(LoginResponse {
        access_token,
        refresh_token,
        user_id,
    })
}

/// Request body for name-login
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct NameLoginRequest {
    #[salvo(schema(example = "user1"))]
    username: String,
    #[salvo(schema(example = "pswd1234"))]
    password: String,
}

/// Response data for login
#[derive(Serialize, ToResponse, ToSchema)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    access_token: String,
    refresh_token: String,
    user_id: String,
}

impl Scribe for LoginResponse {
    fn render(self, res: &mut salvo::Response) {
        res.render(Json(self));
    }
}
