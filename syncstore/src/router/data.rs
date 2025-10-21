use std::sync::Arc;

use salvo::{
    Depot, Router, Scribe, Writer,
    oapi::{
        ToResponse, ToSchema, endpoint,
        extract::{JsonBody, PathParam, QueryParam},
    },
    writing::Json,
};
use serde::{Deserialize, Serialize};

use crate::{backend::Backend, error::ServiceResult, store::Store, types::DataItem};

pub fn create_router() -> Router {
    Router::with_path("{namespace}/{collection}")
        .push(Router::new().post(create_data).get(list_data))
        .push(Router::with_path("{id}").get(get_data))
}

/// List data items with pagination
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
    limit: QueryParam<usize>,
    marker: QueryParam<String, false>,
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
    let limit = match *limit {
        0 => 1,
        n if n > 1000 => 1000,
        n => n,
    };
    let (items, next_marker) = backend.list(collection.as_str(), limit, marker.as_deref())?;
    Ok(ListDataResponse {
        page_info: PageInfo {
            count: items.len(),
            next_marker,
        },
        items,
    })
}

#[derive(Serialize, ToResponse, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListDataResponse {
    // todo might use summary info for list api
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

/// Get a single data item by ID
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
    let store = depot.obtain::<Arc<Store>>()?;
    Ok(store.get(&namespace, &collection, &id)?)
}

/// Create a new data item
#[endpoint(
    status_codes(201, 400, 401),
    request_body(content = String, description = "Data item to create"),
    responses(
        (status_code = 201, description = "Data created successfully", body = String),
        (status_code = 400, description = "Bad request"),
        (status_code = 401, description = "Unauthorized")
    )
)]
async fn create_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    req: JsonBody<serde_json::Value>,
    depot: &mut Depot,
) -> ServiceResult<String> {
    let user_id = depot.get::<String>("user_id")?;
    let store = depot.obtain::<Arc<Store>>()?;
    let item = store.insert(&namespace, &collection, &req.0, user_id.clone())?;
    Ok(item.id)
}

#[endpoint(
    status_codes(200, 400, 401, 404),
    request_body(content = String, description = "Data item to update"),
    responses(
        (status_code = 200, description = "Data updated successfully", body = String),
        (status_code = 400, description = "Bad request"),
        (status_code = 401, description = "Unauthorized"),
        (status_code = 404, description = "Data not found")
    )
)]
async fn update_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    id: PathParam<String>,
    req: JsonBody<serde_json::Value>,
    depot: &mut Depot,
) -> ServiceResult<String> {
    let store = depot.obtain::<Arc<Store>>()?;
    let item = store.update(&namespace, &collection, &id, &req.0)?;
    Ok(item.id)
}
