use std::sync::Arc;

use itertools::Itertools;
use salvo::{
    Depot, Response, Router, Scribe, Writer,
    http::StatusCode,
    oapi::{
        RouterExt, ToResponse, ToSchema, endpoint,
        extract::{PathParam, QueryParam},
    },
    writing::Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ServiceError, ServiceResult},
    router::hpke_wrapper::{HpkeRequest, HpkeResponse},
    store::Store,
    types::{DataItem, DataItemSummary, UserSchema},
};

pub fn create_batch_data_router() -> Router {
    Router::with_path("{namespace}/{collection}")
        .hoop(super::chunk_data_wrapper::check_chunk)
        .push(Router::new().post(batch_get_data)) // todo, deprecated. remove this router in future version.
        .push(Router::with_path("by_ids").post(batch_get_data))
        .push(Router::with_path("by_parent_ids").post(batch_list_data_by_parent))
        .oapi_tag("data")
}

/// Batch list data items by parent IDs
#[endpoint(
    status_codes(200, 403),
    request_body(content = BatchIdRequest, description = "Batch list data items by parent IDs"),
    responses(
        (status_code = 200, description = "Batch list data successfully", body = ListDataResponse),
        (status_code = 400, description = "Bad Request"),
    )
)]
async fn batch_list_data_by_parent(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    req: HpkeRequest<BatchIdRequest>,
    marker: QueryParam<String, false>,
    depot: &mut Depot,
) -> ServiceResult<HpkeResponse<ListDataResponse>> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user = depot.get::<UserSchema>("user_schema")?;
    if req.0.ids.len() > 100 {
        // limit batch get to 100 items to prevent abuse
        Err(ServiceError::RequestError(
            "Batch get limit exceeded: maximum 100 items per request".to_string(),
        ))?;
    }
    let mut items = Vec::new();
    let mut start_parent_id = None;
    let mut start_child_id = None;
    let mut accumulated_size = 0;
    if let Some(marker_str) = marker.as_deref()
        && let Some((p, c)) = marker_str.split_once('.')
    {
        tracing::info!(
            "Batch list data by parent continue: start from marker {}, split into parent_id {} and id {}, will continue to find the position to start",
            marker_str,
            p,
            c
        );
        start_parent_id = Some(p.to_string());
        start_child_id = Some(c.to_string());
    }
    let mut next_p_marker = None;
    let mut next_c_marker = None;
    'parent_loop: for parent_id in req
        .0
        .ids
        .iter()
        .unique()
        .skip_while(|id| start_parent_id.as_ref().is_some_and(|s| s != id.as_str()))
    {
        let mut loop_marker = if start_parent_id.as_ref().is_some_and(|s| s == parent_id.as_str()) {
            start_child_id.take() // 使用后立即 take() 清空，确保下个 Parent 不会误用
        } else {
            None
        };
        loop {
            let (children, marker) =
                store.list_children(&namespace, &collection, parent_id, loop_marker, 100, &user.user_id)?;
            let summary = children.into_iter().map(Into::into).collect::<Vec<DataItemSummary>>();
            for item in &summary {
                accumulated_size += serde_json::to_string(item)
                    .map_err(|e| ServiceError::RequestError(format!("Failed to serialize data item: {e}")))?
                    .len();
                if accumulated_size > 100 * 1024 {
                    next_p_marker = Some(parent_id.clone());
                    next_c_marker = Some(item.id.clone());
                    tracing::info!(
                        "Batch list data by parent truncated: accumulated response size {} bytes exceeds limit, truncating at parent id {}, item id {}",
                        accumulated_size,
                        parent_id,
                        item.id
                    );
                    break 'parent_loop;
                }
                items.push(item.clone());
            }
            if marker.is_none() {
                break;
            }
            loop_marker = marker;
        }
    }
    Ok(HpkeResponse(ListDataResponse {
        page_info: PageInfo {
            count: items.len(),
            next_marker: next_p_marker
                .zip(next_c_marker)
                .map(|(parent_id, id)| format!("{}.{}", parent_id, id)),
        },
        items,
    }))
}

/// Batch get data items by IDs
#[endpoint(
    status_codes(200, 403),
    request_body(content = BatchIdRequest, description = "Batch get data items by IDs"),
    responses(
        (status_code = 200, description = "Batch get data successfully", body = BatchGetDataResponse),
        (status_code = 400, description = "Bad Request"),
    )
)]
async fn batch_get_data(
    namespace: PathParam<String>,
    collection: PathParam<String>,
    req: HpkeRequest<BatchIdRequest>,
    depot: &mut Depot,
) -> ServiceResult<HpkeResponse<BatchGetDataResponse>> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user = depot.get::<UserSchema>("user_schema")?;
    if req.0.ids.len() > 100 {
        // limit batch get to 100 items to prevent abuse
        Err(ServiceError::RequestError(
            "Batch get limit exceeded: maximum 100 items per request".to_string(),
        ))?;
    }
    let mut items = Vec::new();
    let mut truncated = None;
    let mut accumulated_size = 0;
    for id in req.0.ids.iter().unique() {
        if let Ok(item) = store.get(&namespace, &collection, &id, &user.user_id) {
            // simple size check, can be optimized by only counting the body size, or even support streaming response for large data items.
            accumulated_size += serde_json::to_string(&item)
                .map_err(|e| ServiceError::RequestError(format!("Failed to serialize data item: {e}")))?
                .len();
            // todo: make this limit configurable?
            if accumulated_size > 100 * 1024 {
                truncated = Some(id.clone());
                tracing::info!(
                    "Batch get data truncated: accumulated response size {} bytes exceeds limit, truncating at id {}",
                    accumulated_size,
                    id
                );
                break;
            }
            items.push(item);
        }
    }
    Ok(HpkeResponse(BatchGetDataResponse { items, truncated }))
}

#[derive(Deserialize, ToSchema)]
pub struct BatchIdRequest {
    ids: Vec<String>,
}

#[derive(Serialize, ToResponse, ToSchema)]
pub struct BatchGetDataResponse {
    items: Vec<DataItem>,
    truncated: Option<String>,
}

pub fn create_data_router() -> Router {
    Router::with_path("{namespace}/{collection}")
        .hoop(super::chunk_data_wrapper::check_chunk)
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
) -> ServiceResult<HpkeResponse<ListDataResponse>> {
    let user = depot.get::<UserSchema>("user_schema")?;
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
        store.list_children(namespace, collection, parent_id, marker, limit, &user.user_id)?
    } else if let Some(true) = *permission {
        tracing::info!("Listing data [with permission] namespace: {namespace}, collection: {collection}");
        store.list_with_permission(namespace, collection, marker, limit, &user.user_id)?
    } else {
        tracing::info!("Listing data [by owner] namespace: {namespace}, collection: {collection}");
        store.list_by_owner(namespace, collection, marker, limit, &user.user_id)?
    };
    Ok(HpkeResponse(ListDataResponse {
        page_info: PageInfo {
            count: items.len(),
            next_marker,
        },
        items: items.into_iter().map(Into::into).collect(),
    }))
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
) -> ServiceResult<HpkeResponse<DataItem>> {
    let store = depot.obtain::<Arc<Store>>()?;
    let user = depot.get::<UserSchema>("user_schema")?;
    Ok(HpkeResponse(store.get(&namespace, &collection, &id, &user.user_id)?))
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
    req: HpkeRequest<serde_json::Value>,
    depot: &mut Depot,
) -> ServiceResult<HpkeResponse<String>> {
    let user = depot.get::<UserSchema>("user_schema")?;
    let store = depot.obtain::<Arc<Store>>()?;
    let id = store.insert(&namespace, &collection, &req.0, &user.user_id)?;
    Ok(HpkeResponse(id))
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
    req: HpkeRequest<serde_json::Value>,
    depot: &mut Depot,
) -> ServiceResult<HpkeResponse<String>> {
    let user = depot.get::<UserSchema>("user_schema")?;
    let store = depot.obtain::<Arc<Store>>()?;
    let item = store.update(&namespace, &collection, &id, &req.0, &user.user_id)?;
    Ok(HpkeResponse(item.id))
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
    let user = depot.get::<UserSchema>("user_schema")?;
    let store = depot.obtain::<Arc<Store>>()?;
    store.delete(&namespace, &collection, &id, &user.user_id)?;
    resp.status_code(StatusCode::NO_CONTENT);
    Ok(())
}
