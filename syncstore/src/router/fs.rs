use salvo::{Request, Response, Router, handler, http::HeaderValue, prelude::StaticDir};

pub fn create_non_auth_router() -> Router {
    Router::with_path("/public/{*path}").hoop(cache_policies).get(
        StaticDir::new(vec!["./fs/public"])
            .auto_list(true)
            .chunk_size(2 * 1024 * 1024),
    )
}

pub fn create_router() -> Router {
    Router::with_path("/private/{*path}").hoop(cache_policies).get(
        StaticDir::new(vec!["./fs/private"])
            .auto_list(true)
            .chunk_size(2 * 1024 * 1024),
    )
}

#[handler]
fn cache_policies(req: &mut Request, res: &mut Response) {
    let path = req.uri().path();
    match path.rsplit('.').next() {
        Some("jpg") | Some("jpeg") | Some("png") | Some("gif") | Some("svg") | Some("webp") | Some("mp4")
        | Some("mp3") | Some("wav") | Some("flac") => {
            res.headers_mut().insert(
                "Cache-Control",
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
        }
        Some("html") | Some("htm") => {
            res.headers_mut().insert(
                "Cache-Control",
                HeaderValue::from_static("no-cache, no-store, must-revalidate"),
            );
        }
        _ => {
            res.headers_mut()
                .insert("Cache-Control", HeaderValue::from_static("public, max-age=86400"));
        }
    }
}
