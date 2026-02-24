use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use dashmap::DashMap;
use salvo::{Depot, FlowCtrl, Request, Response, handler, http::StatusError};

pub struct UploadStatus {
    _total_chunks: usize,
    received_chunks: BTreeMap<usize, String>,
}

#[handler]
pub async fn check_chunk(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
    ctrl: &mut FlowCtrl,
) -> salvo::Result<()> {
    if let Some(upload_id) = req
        .headers()
        .get("X-Upload-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        && let Some(chunk_index) = req
            .headers()
            .get("X-Chunk-Index")
            .and_then(|h| h.to_str().ok().and_then(|s| s.parse::<usize>().ok()))
        && let Some(_total_chunks) = req
            .headers()
            .get("X-Chunk-Total")
            .and_then(|h| h.to_str().ok().and_then(|s| s.parse::<usize>().ok()))
    {
        let temp_dir = PathBuf::from("./temp_chunks").join(&upload_id);
        if !temp_dir.exists() {
            std::fs::create_dir_all(&temp_dir).ok();
        }
        let chunk_path = temp_dir.join(format!("chunk_{}", chunk_index));
        let body = req
            .payload()
            .await
            .map_err(|_| StatusError::bad_request().brief("Failed to read request payload in chunk"))?
            .to_vec();
        std::fs::write(&chunk_path, body)
            .map_err(|_| StatusError::internal_server_error().brief("Failed to write chunk to temp file"))?;
        tracing::info!(
            "Chunk data saved: upload_id={}, chunk_index={}, total_chunks={}",
            upload_id,
            chunk_index,
            _total_chunks
        );
        let chunk_status = depot
            .obtain::<Arc<DashMap<String, UploadStatus>>>()
            .map_err(|_| StatusError::internal_server_error())?;

        let mut is_completed = false;
        {
            let mut status = chunk_status.entry(upload_id.clone()).or_insert(UploadStatus {
                _total_chunks,
                received_chunks: BTreeMap::new(),
            });
            status
                .received_chunks
                .insert(chunk_index, chunk_path.to_string_lossy().to_string());
            if _total_chunks == status.received_chunks.len() {
                is_completed = true;
            }
        }
        if is_completed {
            tracing::info!("All chunks received for upload_id={}", upload_id);
            let final_data = merge_chunks(&chunk_status, &upload_id);

            tracing::info!(
                "Merged data size for upload_id={}: {} bytes",
                upload_id,
                final_data.len()
            );

            let temp_dir = PathBuf::from("./temp_chunks").join(&upload_id);
            std::fs::remove_dir_all(&temp_dir).ok();
            chunk_status.remove(&upload_id);

            req.headers_mut()
                .insert("Content-Length", final_data.len().to_string().parse().unwrap());
            req.replace_body(salvo::http::ReqBody::Once(final_data.into()));

            ctrl.call_next(req, depot, res).await;
            return Ok(());
        } else {
            res.status_code(salvo::http::StatusCode::ACCEPTED);
            ctrl.skip_rest();
            return Ok(());
        }
    }
    tracing::info!("Not a chunk upload request, continue normal processing");
    ctrl.call_next(req, depot, res).await;
    Ok(())
}
fn merge_chunks(state: &DashMap<String, UploadStatus>, upload_id: &str) -> Vec<u8> {
    let status = state.get(upload_id).unwrap();
    let mut combined = Vec::new();

    for (_, path) in &status.received_chunks {
        let mut f = std::fs::File::open(path).unwrap();
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buffer).unwrap();
        combined.extend(buffer);
    }
    combined
}
