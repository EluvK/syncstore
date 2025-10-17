use std::sync::Arc;

use salvo::{
    Depot, Router, Scribe, Writer,
    oapi::{
        RouterExt, ToResponse, ToSchema, endpoint,
        extract::{JsonBody, PathParam},
    },
    writing::Json,
};
use serde::{Deserialize, Serialize};

use crate::{backend::Backend, error::ServiceResult, store::Store, types::DataItem};

pub fn create_router() -> Router {
    Router::with_path("{namespace}/{collection}")
        .push(Router::new().post(list_data))
        .push(Router::with_path("{id}").get(get_data))
}

#[endpoint(
    status_codes(200, 401),
    request_body(content = ListDataRequest, description = "List data items with pagination"),
    responses(
        (status_code = 200, description = "List data successfully", body = ListDataResponse),
        (status_code = 401, description = "Unauthorized")
    )
)]
async fn list_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    req: JsonBody<ListDataRequest>,
    depot: &mut Depot,
) -> ServiceResult<ListDataResponse> {
    tracing::info!(
        "Listing data in namespace: {}, collection: {}",
        namespace.as_str(),
        collection.as_str()
    );
    let data_manager = depot.obtain::<Arc<Store>>()?.data_manager.clone();
    let backend = data_manager.backend_for(namespace.as_str())?;
    // limit must be positive
    let marker = req.marker.as_deref();
    let limit = match req.limit {
        0 => 1,
        n if n > 1000 => 1000,
        n => n,
    };
    let (items, next_marker) = backend.list(collection.as_str(), limit, marker)?;
    Ok(ListDataResponse {
        page_info: PageInfo {
            count: items.len(),
            next_marker,
        },
        items,
    })
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListDataRequest {
    marker: Option<String>,
    limit: usize, // default 100, max 1000
}

#[derive(Serialize, ToResponse, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListDataResponse {
    items: Vec<DataItem>,
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
    status_codes(200, 401, 404),
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
    let data_manager = depot.obtain::<Arc<Store>>()?.data_manager.clone();
    let backend = data_manager.backend_for(namespace.as_str())?;
    Ok(backend.get(&collection, &id)?)
}

impl Scribe for DataItem {
    fn render(self, res: &mut salvo::Response) {
        res.render(Json(self));
    }
}
