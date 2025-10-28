use std::sync::Arc;

use salvo::{Depot, Response, Router, Writer, handler, oapi::extract::JsonBody};
use serde::Deserialize;

use crate::{error::ServiceResult, store::Store};

pub fn create_router() -> Router {
    Router::new().push(Router::with_path("register").post(register))
}

#[handler]
async fn register(body: JsonBody<RegisterRequest>, depot: &mut Depot, _resp: &mut Response) -> ServiceResult<()> {
    let store = depot.obtain::<Arc<Store>>()?;
    store.create_user(&body.username, &body.password)?;
    Ok(())
}

/// Request body for user registration
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterRequest {
    username: String,
    password: String,
}
