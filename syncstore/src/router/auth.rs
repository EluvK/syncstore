use salvo::{
    Router, Scribe, Writer,
    oapi::{ToResponse, ToSchema, endpoint, extract::JsonBody},
    writing::Json,
};
use serde::{Deserialize, Serialize};

use crate::error::ServiceResult;

pub fn create_router() -> Router {
    Router::new()
}

pub fn create_non_auth_router() -> Router {
    Router::new()
}

#[endpoint(
    status_codes(200, 401),
    request_body(content = NameLoginRequest, description = "Login by username and password"),
    responses(
        (status_code = 200, description = "Login successful", body = LoginResponse),
        (status_code = 401, description = "Unauthorized")
    )
)]
async fn login(req: JsonBody<NameLoginRequest>) -> ServiceResult<LoginResponse> {
    // Handle login logic here

    todo!()
}

// -- schema definitions

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct NameLoginRequest {
    #[salvo(schema(example = "user1"))]
    username: String,
    #[salvo(schema(example = "password"))]
    password: String,
}

#[derive(Serialize, ToResponse, ToSchema)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    access_token: String,
    user_id: String,
}

impl Scribe for LoginResponse {
    fn render(self, res: &mut salvo::Response) {
        res.render(Json(self));
    }
}
