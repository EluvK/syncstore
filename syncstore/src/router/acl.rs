use std::sync::Arc;

use salvo::{
    Depot, Router, Writer,
    oapi::{
        RouterExt, ToSchema, endpoint,
        extract::{JsonBody, PathParam},
    },
};
use serde::Deserialize;

use crate::{error::ServiceResult, store::Store};

pub fn create_router() -> Router {
    Router::with_path("{namespace}/{collection}")
        .post(create_acl)
        .oapi_tag("acl")
}

/// Create a new ACL for specified resources
#[endpoint(
    status_codes(201, 400, 401),
    request_body(content = CreateAclRequest, description = "Create a new ACL"),
    responses(
        (status_code = 201, description = "ACL created successfully"),
        (status_code = 400, description = "Bad Request"),
        (status_code = 401, description = "Unauthorized")
    )
)]
async fn create_acl(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    req: JsonBody<CreateAclRequest>,
    depot: &mut Depot,
) -> ServiceResult<()> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user = depot.get::<String>("user_id")?;
    let acl = todo!();
    store.create_acl((namespace.as_str(), collection.as_str()), acl, user);
    tracing::info!("create_acl called");
    Ok(())
}

#[derive(Deserialize, ToSchema)]
pub struct CreateAclRequest {
    // pub
    pub resource: String,
    pub permissions: Vec<String>,
}
