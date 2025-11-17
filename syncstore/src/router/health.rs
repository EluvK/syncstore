use salvo::{Router, handler};

pub fn create_router() -> Router {
    Router::with_path("health").get(get_health)
}

#[handler]
fn get_health() -> &'static str {
    "OK"
}
