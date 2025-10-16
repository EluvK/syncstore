use std::sync::Arc;

use salvo::{
    Depot, Router, Scribe, Writer,
    oapi::{ToResponse, ToSchema, endpoint, extract::PathParam},
    writing::Json,
};
use serde::{Deserialize, Serialize};

use crate::{error::ServiceResult, store::Store};

pub fn create_router() -> Router {
    Router::with_path("<namespace>/<collection>")
        .push(Router::new().post(list_data))
        .push(Router::with_path("<id>").get(get_data))
}

#[endpoint(
    status_codes(200, 401),
    responses(
        (status_code = 200, description = "List data successfully", body = ListDataResponse),
        (status_code = 401, description = "Unauthorized")
    )
)]
async fn list_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    depot: &mut Depot,
) -> ServiceResult<ListDataResponse> {
    let data_manager = depot.obtain::<Arc<Store>>()?.data_manager.clone();
    // data_manager.backend_for(namespace)
    todo!()
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListDataRequest {
    marker: Option<String>,
    limit: Option<usize>, // default 100, max 1000
}

#[derive(Serialize, ToResponse, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListDataResponse {
    items: Vec<String>, // todo should this be serializable data item, or some concrete type T
    page_info: PageInfo,
}

#[derive(Deserialize, Serialize, ToResponse, ToSchema)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    count: usize,
    next_marker: Option<String>,
}

impl Scribe for ListDataResponse {
    fn render(self, res: &mut salvo::Response) {
        res.render(Json(self));
    }
}

#[endpoint(
    status_codes(200, 401),
    responses(
        (status_code = 200, description = "Get data successfully", body = DataItem),
        (status_code = 401, description = "Unauthorized"),
        (status_code = 404, description = "Data not found")
    )
)]
async fn get_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    id: PathParam<String>,
    depot: &mut Depot,
) -> ServiceResult<DataItem> {
    todo!()
}

#[derive(Serialize, ToResponse, ToSchema)]
#[serde(rename_all = "camelCase")]
struct DataItem {}

impl Scribe for DataItem {
    fn render(self, res: &mut salvo::Response) {
        res.render(Json(self));
    }
}
