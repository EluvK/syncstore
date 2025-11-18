use std::sync::Arc;

use salvo::{
    Depot, Router, Scribe, Writer,
    oapi::{
        RouterExt, ToResponse, ToSchema, endpoint,
        extract::{JsonBody, PathParam},
    },
};
use serde::{Deserialize, Serialize};

use crate::{
    error::ServiceResult,
    store::Store,
    types::{AccessControl, Permission},
};

pub fn create_router() -> Router {
    Router::with_path("{namespace}/{collection}")
        .push(
            Router::with_path("{id}")
                .get(get_acl)
                .post(create_acl)
                .delete(delete_acl),
        )
        .oapi_tag("acl")
}

/// Create a new ACL for specified resources
#[endpoint(
    status_codes(201, 400, 403),
    request_body(content = CreateAclRequest, description = "Create a new ACL"),
    responses(
        (status_code = 201, description = "ACL created successfully"),
        (status_code = 400, description = "Bad Request"),
        (status_code = 403, description = "FORBIDDEN")
    )
)]
async fn create_acl(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    id: PathParam<String>,
    req: JsonBody<CreateAclRequest>,
    depot: &mut Depot,
) -> ServiceResult<()> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user = depot.get::<String>("user_id")?;
    let acl = AccessControl {
        data_id: id.to_string(),
        permissions: req.permissions.clone(),
    };
    store.create_acl((namespace.as_str(), collection.as_str()), acl, user)?;
    tracing::info!("create_acl for data {}", id.as_str());
    Ok(())
}

#[derive(Deserialize, ToSchema)]
pub struct CreateAclRequest {
    permissions: Vec<Permission>,
}

/// Get ACL for specified resources
#[endpoint(
    status_codes(200, 403, 404),
    responses(
        (status_code = 200, description = "Get ACL successfully", body = GetAclResponse),
        (status_code = 403, description = "FORBIDDEN"),
        (status_code = 404, description = "Not Found")
    )
)]
async fn get_acl(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    id: PathParam<String>,
    depot: &mut Depot,
) -> ServiceResult<GetAclResponse> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user = depot.get::<String>("user_id")?;
    let acl = store.get_acl((namespace.as_str(), collection.as_str()), id.as_str(), user)?;
    tracing::info!("get_acl for data {}", id.as_str());
    Ok(GetAclResponse {
        permissions: acl.permissions,
    })
}

#[derive(Serialize, ToSchema, ToResponse)]
pub struct GetAclResponse {
    permissions: Vec<Permission>,
}

impl Scribe for GetAclResponse {
    fn render(self, res: &mut salvo::Response) {
        res.render(salvo::writing::Json(self));
    }
}

/// Delete ACL for specified resources
#[endpoint(
    status_codes(204, 403, 404),
    responses(
        (status_code = 204, description = "ACL deleted successfully"),
        (status_code = 403, description = "FORBIDDEN"),
        (status_code = 404, description = "Not Found")
    )
)]
async fn delete_acl(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    id: PathParam<String>,
    depot: &mut Depot,
) -> ServiceResult<()> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user = depot.get::<String>("user_id")?;
    store.delete_acl((namespace.as_str(), collection.as_str()), id.as_str(), user)?;
    tracing::info!("delete_acl for data {}", id.as_str());
    Ok(())
}
