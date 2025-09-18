use salvo::{
    Depot, FlowCtrl, Request, Response, Router, handler,
    jwt_auth::{ConstDecoder, HeaderFinder, QueryFinder},
    prelude::{JwtAuth, JwtAuthDepotExt, JwtAuthState},
};

use crate::{
    config::ServiceConfig,
    error::{ServiceError, ServiceResult},
    utils::jwt::JwtClaims,
};

pub fn create_router(config: &ServiceConfig) -> Router {
    let auth_handler: JwtAuth<JwtClaims, _> =
        JwtAuth::new(ConstDecoder::from_secret(config.jwt.access_secret.as_bytes()))
            .finders(vec![
                Box::new(HeaderFinder::new()),
                Box::new(QueryFinder::new("jwt_token")),
            ])
            .force_passed(true);

    let non_auth_router = Router::new();
    let auth_router = Router::new().hoop(auth_handler).hoop(jwt_to_user);
    Router::new().push(auth_router).push(non_auth_router)
}

// check the jwt token from request, convert to user profile.
#[handler]
async fn jwt_to_user(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
    ctrl: &mut FlowCtrl,
) -> ServiceResult<()> {
    match (depot.jwt_auth_state(), depot.jwt_auth_data::<JwtClaims>()) {
        (JwtAuthState::Authorized, Some(jwt_token)) => {
            let claim = jwt_token.claims.clone();
            if claim.is_expired() {
                tracing::info!("Unauthorized: JWT token expired");
                res.render(ServiceError::Unauthorized("JWT token expired".to_string()));
                ctrl.skip_rest();
                return Ok(());
            }
            // todo fetch user profile from db.

            tracing::info!("Authorized.");
        }
        (_, None) => {
            tracing::info!("Unauthorized: No JWT token found");
            res.render(ServiceError::Unauthorized("No JWT token found".to_string()));
            ctrl.skip_rest();
        }
        _ => {
            tracing::info!("Unauthorized: Invalid JWT token");
            res.render(ServiceError::Unauthorized("Invalid JWT token".to_string()));
            ctrl.skip_rest();
        }
    }

    Ok(())
}
