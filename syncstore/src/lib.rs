//! SyncStore library - lightweight core abstractions and an in-memory backend for prototyping.

use salvo::{
    oapi::{OpenApi, SecurityScheme, security::Http},
    prelude::*,
};

use std::sync::Arc;

pub mod backend;
pub mod components;
pub mod config;
pub mod error;
pub mod router;
pub mod store;
pub mod types;
pub mod utils;

// pub use crate::backend::Backend;
// pub use crate::store::Store;
// pub use crate::types::{Id, Meta};

pub async fn init_service(store: Arc<store::Store>, config: &config::ServiceConfig) -> anyhow::Result<()> {
    utils::jwt::set_jwt_config(&config.jwt);

    let api_router = Router::new().push(Router::with_path("api").push(router::create_router(config, store.clone())));
    let admin_router = Router::new().push(Router::with_path("admin").push(router::admin_router(store)));

    // make the openapi doc schema names more readable
    salvo::oapi::naming::set_namer(
        salvo::oapi::naming::FlexNamer::new()
            .short_mode(true)
            .generic_delimiter('_', '_'),
    );
    let doc = OpenApi::new("SyncStore API", "0.1.0")
        .add_security_scheme(
            "bearer",
            SecurityScheme::Http(Http::new(salvo::oapi::security::HttpAuthScheme::Bearer).bearer_format("JWT")),
        )
        .merge_router(&api_router);
    let router = api_router
        .unshift(doc.into_router("/api-doc/openapi.json"))
        .unshift(SwaggerUi::new("/api-doc/openapi.json").into_router("/swagger-ui"));
    tokio::join!(
        async {
            let acceptor = TcpListener::new(config.admin_address.clone()).bind().await;
            let service = Service::new(admin_router);
            tracing::info!("Admin server started at {}", &config.admin_address);
            Server::new(acceptor).serve(service).await
        },
        async {
            let acceptor = TcpListener::new(config.address.clone()).bind().await;
            let service = Service::new(router).hoop(Logger::new());
            tracing::info!("Server started at {}", &config.address);
            Server::new(acceptor).serve(service).await
        }
    );
    Ok(())
}
