mod acl;
mod admin;
mod auth;
mod data;
mod fs;
mod health;
mod user;

use std::sync::Arc;

use salvo::{
    Depot, FlowCtrl, Request, Response, Router, affix_state, handler,
    jwt_auth::{ConstDecoder, HeaderFinder, QueryFinder},
    oapi::{RouterExt, SecurityRequirement},
    prelude::{JwtAuth, JwtAuthDepotExt, JwtAuthState},
};

use crate::{
    config::ServiceConfig,
    error::{ServiceError, ServiceResult},
    store::Store,
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
        .push(Router::with_path("fs").push(fs::create_router()))
        .push(health::create_router());
    let auth_router = Router::new()
        .hoop(auth_handler)
        .hoop(jwt_to_user)
        .push(Router::with_path("acl").push(acl::create_router()))
        .push(Router::with_path("auth").push(auth::create_router()))
        .push(Router::with_path("data").push(data::create_router()))
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
            depot.insert("user_id", user_id.clone());
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
