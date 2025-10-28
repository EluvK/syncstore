use std::sync::Arc;

use salvo::{
    Depot, Router, Writer,
    oapi::{
        RouterExt, ToSchema, endpoint,
        extract::{JsonBody, PathParam},
    },
};
use serde::Deserialize;

use crate::{error::ServiceResult, store::Store, types::AccessControl};

pub fn create_router() -> Router {
    Router::with_path("{namespace}/{collection}")
        .post(create_acl)
        .oapi_tag("acl")
}

/// Create a new ACL for specified resources
#[endpoint(
    status_codes(201, 400, 401),
    request_body(content = AccessControl, description = "Create a new ACL"),
    responses(
        (status_code = 201, description = "ACL created successfully"),
        (status_code = 400, description = "Bad Request"),
        (status_code = 401, description = "Unauthorized")
    )
)]
async fn create_acl(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    req: JsonBody<AccessControl>,
    depot: &mut Depot,
) -> ServiceResult<()> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user = depot.get::<String>("user_id")?;
    store.create_acl((namespace.as_str(), collection.as_str()), req.into_inner(), user)?;
    tracing::info!("create_acl called");
    Ok(())
}
