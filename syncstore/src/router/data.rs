use std::sync::Arc;

use salvo::{
    Depot, Response, Router, Scribe, Writer,
    http::StatusCode,
    oapi::{
        RouterExt, ToResponse, ToSchema, endpoint,
        extract::{JsonBody, PathParam, QueryParam},
    },
    writing::Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::ServiceResult,
    store::Store,
    types::{DataItem, DataItemSummary},
};

pub fn create_router() -> Router {
    Router::with_path("{namespace}/{collection}")
        .push(Router::new().post(create_data).get(list_data))
        .push(
            Router::with_path("{id}")
                .get(get_data)
                .post(update_data)
                .delete(delete_data),
        )
        .oapi_tag("data")
}

/// List data items summary with pagination
#[endpoint(
    status_codes(200, 403),
    responses(
        (status_code = 200, description = "List data successfully", body = ListDataResponse),
        (status_code = 403, description = "FORBIDDEN")
    )
)]
async fn list_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    parent_id: QueryParam<String, false>,
    permission: QueryParam<bool, false>,
    marker: QueryParam<String, false>,
    limit: QueryParam<usize>,
    depot: &mut Depot,
) -> ServiceResult<ListDataResponse> {
    let user = depot.get::<String>("user_id")?;
    let namespace = namespace.as_str();
    let collection = collection.as_str();
    let marker = marker.clone();
    // limit must be positive
    let limit = match *limit {
        0 => 1,
        n if n > 1000 => 1000,
        n => n,
    };
    let store = depot.obtain::<Arc<Store>>()?;
    let (items, next_marker) = if let Some(parent_id) = parent_id.as_deref() {
        tracing::info!("Listing data [children] namespace: {namespace}, collection: {collection}");
        store.list_children(namespace, collection, parent_id, marker, limit, user)?
    } else if let Some(true) = *permission {
        tracing::info!("Listing data [with permission] namespace: {namespace}, collection: {collection}");
        store.list_with_permission(namespace, collection, marker, limit, user)?
    } else {
        tracing::info!("Listing data [by owner] namespace: {namespace}, collection: {collection}");
        store.list_by_owner(namespace, collection, marker, limit, user)?
    };
    Ok(ListDataResponse {
        page_info: PageInfo {
            count: items.len(),
            next_marker,
        },
        items: items.into_iter().map(Into::into).collect(),
    })
}

#[derive(Serialize, ToResponse, ToSchema)]
struct ListDataResponse {
    items: Vec<DataItemSummary>,
    page_info: PageInfo,
}

#[derive(Deserialize, Serialize, ToResponse, ToSchema)]
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
    status_codes(200, 403, 404),
    responses(
        (status_code = 200, description = "Get data successfully", body = DataItem),
        (status_code = 403, description = "FORBIDDEN"),
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
    let user = depot.get::<String>("user_id")?;
    Ok(store.get(&namespace, &collection, &id, user)?)
}

/// Create a new data item
#[endpoint(
    status_codes(201, 400, 403),
    request_body(content = serde_json::Value, description = "Data item to create"),
    responses(
        (status_code = 201, description = "Data created successfully", body = String),
        (status_code = 400, description = "Bad request"),
        (status_code = 403, description = "FORBIDDEN")
    )
)]
async fn create_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    req: JsonBody<serde_json::Value>,
    depot: &mut Depot,
) -> ServiceResult<String> {
    let user = depot.get::<String>("user_id")?;
    let store = depot.obtain::<Arc<Store>>()?;
    let id = store.insert(&namespace, &collection, &req.0, user)?;
    Ok(id)
}

/// Update an existing data item
#[endpoint(
    status_codes(200, 400, 403, 404),
    request_body(content = serde_json::Value, description = "Data item to update"),
    responses(
        (status_code = 200, description = "Data updated successfully", body = String),
        (status_code = 400, description = "Bad request"),
        (status_code = 403, description = "FORBIDDEN"),
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
    let user = depot.get::<String>("user_id")?;
    let store = depot.obtain::<Arc<Store>>()?;
    let item = store.update(&namespace, &collection, &id, &req.0, user)?;
    Ok(item.id)
}

/// Delete a data item
#[endpoint(
    status_codes(204, 403, 404),
    responses(
        (status_code = 204, description = "Data deleted successfully"),
        (status_code = 403, description = "FORBIDDEN"),
        (status_code = 404, description = "Data not found")
    )
)]
async fn delete_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    id: PathParam<String>,
    depot: &mut Depot,
    resp: &mut Response,
) -> ServiceResult<()> {
    let user = depot.get::<String>("user_id")?;
    let store = depot.obtain::<Arc<Store>>()?;
    store.delete(&namespace, &collection, &id, user)?;
    resp.status_code(StatusCode::NO_CONTENT);
    Ok(())
}
