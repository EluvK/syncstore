mod acl;
mod admin;
mod auth;
mod data;
mod fs;
mod health;
mod hpke_wrapper;
mod user;

use std::sync::Arc;

use base64::Engine;
use salvo::{
    Depot, FlowCtrl, Request, Response, Router, affix_state, handler,
    http::{HeaderValue, ResBody},
    jwt_auth::{ConstDecoder, HeaderFinder, QueryFinder},
    oapi::{RouterExt, SecurityRequirement},
    prelude::{JwtAuth, JwtAuthDepotExt, JwtAuthState},
};

use crate::{
    config::ServiceConfig,
    error::{ServiceError, ServiceResult},
    store::Store,
    types::UserSchema,
    utils::jwt::JwtClaims,
};

pub fn create_router(config: &ServiceConfig, store: Arc<Store>) -> Router {
    let auth_handler: JwtAuth<JwtClaims, _> =
        JwtAuth::new(ConstDecoder::from_secret(config.jwt.access_secret.as_bytes()))
            .finders(vec![
                Box::new(HeaderFinder::new()),
                Box::new(QueryFinder::new("jwt_token")),
            ])
            .force_passed(true);

    let non_auth_router = Router::new()
        .push(Router::with_path("auth").push(auth::create_non_auth_router()))
        .push(Router::with_path("fs").push(fs::create_non_auth_router()))
        .push(health::create_router());
    let auth_router = Router::new()
        .hoop(auth_handler)
        .hoop(jwt_to_user)
        .hoop(header_makeup)
        // .hoop(hpke)
        .push(Router::with_path("acl").push(acl::create_router()))
        .push(Router::with_path("auth").push(auth::create_router()))
        .push(Router::with_path("data").push(data::create_router()))
        .push(Router::with_path("fs").push(fs::create_router()))
        .push(Router::with_path("user").push(user::create_router()))
        .oapi_security(SecurityRequirement::new("bearer", vec!["bearer"]));
    Router::new()
        .hoop(affix_state::inject(store))
        .push(auth_router)
        .push(non_auth_router)
}

pub fn admin_router(store: Arc<Store>) -> Router {
    Router::new()
        .hoop(affix_state::inject(store))
        .push(admin::create_router())
}

// check the jwt token from request, convert to user profile.
#[handler]
async fn jwt_to_user(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
    ctrl: &mut FlowCtrl,
) -> ServiceResult<()> {
    match (
        depot.jwt_auth_state(),
        depot.jwt_auth_data::<JwtClaims>(),
        depot.jwt_auth_error(),
    ) {
        (JwtAuthState::Authorized, Some(jwt_token), _) => {
            let claim = jwt_token.claims.clone();
            if claim.is_expired() {
                tracing::info!("Unauthorized: JWT token expired");
                res.render(ServiceError::Unauthorized("JWT token expired".to_string()));
                ctrl.skip_rest();
                return Ok(());
            }
            let store = depot.obtain::<Arc<Store>>()?;
            let user_id = claim.sub.clone();
            let Ok(user) = store.get_user(&user_id) else {
                tracing::info!("Unauthorized: User not found");
                res.render(ServiceError::Unauthorized("User not found".to_string()));
                ctrl.skip_rest();
                return Ok(());
            };
            tracing::info!("Authorized. user:{}({})", user.username, user_id);
            depot.insert("user_schema", user.clone());
            req.extensions_mut().insert(user.clone());
            ctrl.call_next(req, depot, res).await;
        }
        (_, _, Some(jwt_error)) => {
            tracing::info!("Unauthorized: JWT auth error: {}", jwt_error);
            res.render(ServiceError::Unauthorized(format!("JWT auth error: {}", jwt_error)));
            ctrl.skip_rest();
        }
        (_, _, _) => {
            tracing::info!("Unauthorized: Invalid JWT token");
            res.render(ServiceError::Unauthorized("Invalid JWT token".to_string()));
            ctrl.skip_rest();
        }
    }

    Ok(())
}

#[handler]
async fn header_makeup(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
    ctrl: &mut FlowCtrl,
) -> ServiceResult<()> {
    // if "X-Enc" and "X-Session-PubKey" headers exist, make it into res headers as well
    // get the url path and make into "X-Path" header
    if let Some(x_enc) = req.headers().get("X-Enc") {
        res.headers_mut().insert("X-Enc", x_enc.clone());
    }
    if let Some(x_session_pubkey) = req.headers().get("X-Session-PubKey") {
        res.headers_mut().insert("X-Session-PubKey", x_session_pubkey.clone());
    }
    let path = req.uri().path().to_string();
    // req.headers_mut().insert(
    //     "X-Path",
    //     HeaderValue::from_str(&path).unwrap_or_else(|_| HeaderValue::from_static("")),
    // );
    res.headers_mut().insert(
        "X-Path",
        HeaderValue::from_str(&path).unwrap_or_else(|_| HeaderValue::from_static("")),
    );

    ctrl.call_next(req, depot, res).await;
    Ok(())
}

// handle secure headers
#[handler]
async fn hpke(req: &mut Request, res: &mut Response, depot: &mut Depot, ctrl: &mut FlowCtrl) -> ServiceResult<()> {
    // check if the request has the HPKE headers, fetch and decode them
    let Some((encapped_key, session_pubkey)) = req
        .headers()
        .get("X-Enc")
        .and_then(|v| v.to_str().ok())
        .zip(req.headers().get("X-Session-PubKey").and_then(|v| v.to_str().ok()))
        .and_then(|(e, s)| {
            let e_dec = base64::engine::general_purpose::STANDARD.decode(e).ok()?;
            let s_dec = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
            Some((e_dec, s_dec))
        })
    else {
        ctrl.call_next(req, depot, res).await;
        return Ok(());
    };
    tracing::info!("HPKE: headers found in path: {}", req.uri().path());
    let Ok(user_schema) = depot.get::<UserSchema>("user_schema") else {
        tracing::warn!("HPKE: user_schema not found in depot");
        res.render(ServiceError::Unauthorized("user_schema not found".to_string()));
        ctrl.skip_rest();
        return Ok(());
    };
    let aad = req.uri().path().as_bytes().to_vec();

    // read the request body as ciphertext
    let Ok(ciphertext) = req.payload().await else {
        tracing::warn!("HPKE: Failed to read request payload");
        res.render(ServiceError::Unauthorized("Failed to read request payload".to_string()));
        ctrl.skip_rest();
        return Ok(());
    };
    let plaintext = crate::utils::hpke::decrypt_data(ciphertext, &encapped_key, &user_schema.secret_key, &aad)?;
    // replace the request body with the decrypted plaintext
    req.replace_body(plaintext.into());

    ctrl.call_next(req, depot, res).await;

    // encrypt the response body
    let body = res.take_body();
    if let salvo::http::ResBody::Once(bytes) = body {
        let (new_enc_key, encrypted_body) = crate::utils::hpke::encrypt_data(&bytes, &session_pubkey, &aad)?;
        res.headers_mut().insert(
            "X-Enc",
            HeaderValue::from_str(&base64::engine::general_purpose::STANDARD.encode(&new_enc_key)).map_err(|e| {
                tracing::warn!("HPKE: Invalid X-Enc header value: {}", e);
                ServiceError::InternalServerError("Invalid X-Enc header value".to_string())
            })?,
        );
        res.headers_mut()
            .insert("Content-Type", HeaderValue::from_static("application/octet-stream"));
        res.replace_body(ResBody::Once(encrypted_body.into()));
    } else {
        res.replace_body(body);
        tracing::warn!("HPKE: other body is not supported for encryption");
    }

    Ok(())
}
