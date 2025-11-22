use std::sync::Arc;

use salvo::{
    Depot, Router, Writer,
    oapi::{
        RouterExt, ToResponse, ToSchema, endpoint,
        extract::{JsonBody, PathParam},
    },
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ServiceError, ServiceResult},
    store::Store,
    types::UserSchema,
};

pub fn create_router() -> Router {
    Router::with_path("{id}")
        .get(get_self_user)
        .post(update_user)
        .oapi_tag("user")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, ToResponse)]
pub struct UserProfile {
    pub user_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

impl salvo::Scribe for UserProfile {
    fn render(self, res: &mut salvo::Response) {
        res.render(salvo::writing::Json(self));
    }
}

impl UserProfile {
    fn from_user_schema(user_id: String, user_schema: &UserSchema) -> Self {
        UserProfile {
            user_id,
            name: user_schema.username.clone(),
            avatar_url: user_schema.avatar_url.clone(),
        }
    }
}

/// Get user profile by ID
#[endpoint(
    status_codes(200, 403, 404),
    responses(
        (status_code = 200, description = "Get user profile successfully", body = UserProfile),
        (status_code = 403, description = "FORBIDDEN"),
    )
)]
async fn get_self_user(id: PathParam<String>, depot: &mut Depot) -> ServiceResult<UserProfile> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user_schema = store.get_user(&id)?;
    let user = UserProfile::from_user_schema(id.to_string(), &user_schema);
    Ok(user)
}

/// Update user profile by ID
#[endpoint(
    status_codes(200, 400, 403, 404),
    responses(
        (status_code = 200, description = "Update user profile successfully", body = UserProfile),
        (status_code = 400, description = "BAD REQUEST"),
        (status_code = 403, description = "FORBIDDEN"),
    )
)]
async fn update_user(
    id: PathParam<String>,
    req: JsonBody<UpdateUserProfile>,
    depot: &mut Depot,
) -> ServiceResult<UserProfile> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user_id = depot.get::<String>("user_id")?.to_string();
    if user_id != *id {
        return Err(ServiceError::Forbidden(
            "Cannot update other user's profile".to_string(),
        ));
    }
    let user_schema = store.get_user(&user_id)?;
    let mut updated_schema = user_schema.clone();
    if let Some(name) = &req.0.name {
        updated_schema.username = name.clone();
    }
    if let Some(password) = &req.0.password {
        updated_schema.password = password.clone();
    }
    if let Some(avatar_url) = &req.0.avatar_url {
        updated_schema.avatar_url = Some(avatar_url.clone());
    }
    store.update_user(&user_id, &updated_schema)?;
    let updated_user = store.get_user(&user_id)?;
    let updated_user = UserProfile::from_user_schema(user_id, &updated_user);
    Ok(updated_user)
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateUserProfile {
    pub name: Option<String>,
    pub password: Option<String>,
    pub avatar_url: Option<String>,
}
