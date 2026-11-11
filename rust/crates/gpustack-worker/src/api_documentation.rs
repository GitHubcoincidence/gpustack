//! Shared metadata and authentication schemes for the generated Worker OpenAPI document.

use utoipa::openapi::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi as OpenApiDerive};

struct SecuritySchemes;

impl Modify for SecuritySchemes {
    fn modify(&self, openapi: &mut OpenApi) {
        let components = openapi.components.get_or_insert_default();

        let mut bearer = Http::new(HttpAuthScheme::Bearer);
        bearer.description = Some(
            "Worker resource endpoints require the per-Worker token. Managed inference accepts the per-Worker token or the Cluster registration token."
                .to_owned(),
        );
        components.add_security_scheme("bearer_auth", SecurityScheme::Http(bearer));

        let mut api_key = ApiKeyValue::new("X-API-Key");
        api_key.description = Some(
            "Alternative header transport for the same endpoint-specific token accepted by Authorization: Bearer."
                .to_owned(),
        );
        components.add_security_scheme(
            "api_key_auth",
            SecurityScheme::ApiKey(ApiKey::Header(api_key)),
        );
    }
}

#[derive(OpenApiDerive)]
#[openapi(
    modifiers(&SecuritySchemes),
    tags(
        (name = "Probes", description = "Worker liveness, readiness, and build identity"),
        (name = "Resources", description = "Authenticated local model resource inspection"),
        (name = "Logs", description = "Authenticated model instance log discovery and streaming"),
        (name = "Configuration", description = "Authenticated non-sensitive Worker configuration"),
        (name = "Inference", description = "Authenticated proxy to a healthy managed vLLM instance")
    )
)]
struct WorkerApiDocument;

pub(crate) fn base_document() -> OpenApi {
    let mut document = WorkerApiDocument::openapi();
    document.info.title = "GPUStack Rust Worker API".to_owned();
    document.info.version = env!("CARGO_PKG_VERSION").to_owned();
    document.info.description = Some(
        "OpenAPI contract for the Rust Worker HTTP listener. Prometheus metrics are served on the separately configured metrics listener and are documented in the Worker API guide."
            .to_owned(),
    );
    document
}
