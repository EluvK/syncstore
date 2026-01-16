use std::fmt;

use base64::Engine;
use salvo::{
    Extractible, Request, Response, Scribe, Writer, async_trait,
    extract::Metadata,
    http::{HeaderValue, StatusError, header::CONTENT_TYPE},
    oapi::{
        Components, Content, EndpointArgRegister, EndpointOutRegister, Operation, RequestBody, ToRequestBody, ToSchema,
    },
};
use serde::{Deserialize, Serialize};

use crate::{types::UserSchema, utils::hpke};

/// HPKE JSON body extractor
#[derive(ToSchema)]
pub struct HpkeRequest<T>(pub T);

impl<'ex, T> Extractible<'ex> for HpkeRequest<T>
where
    T: serde::de::DeserializeOwned + Send,
{
    fn metadata() -> &'static Metadata {
        static METADATA: Metadata = Metadata::new("HPKE JSON body");
        &METADATA
    }

    async fn extract(req: &'ex mut Request) -> Result<Self, impl Writer + Send + fmt::Debug + 'static> {
        let final_bytes = if let Some(encapped_key) = req.headers().get_base64("X-Enc") {
            let bytes = req
                .payload()
                .await
                .map_err(|e| StatusError::bad_request().brief(e.to_string()))?
                .to_vec();
            tracing::info!("HPKE[req]: HPKE headers found, decrypting...");
            let user_schema = req
                .extensions_mut()
                .get::<UserSchema>()
                .ok_or_else(|| StatusError::unauthorized().brief("user_schema not found"))?
                .clone();
            let aad = req.uri().path().as_bytes().to_vec();
            let decrypted_bytes = hpke::decrypt_data(&bytes, &encapped_key, &user_schema.secret_key, &aad)
                .map_err(|e| StatusError::bad_request().brief(e.to_string()))?;
            decrypted_bytes
        } else {
            tracing::info!("HPKE[req]: no HPKE headers found, treat as plain JSON");
            req.payload()
                .await
                .map_err(|e| StatusError::bad_request().brief(e.to_string()))?
                .to_vec()
        };
        let value = serde_json::from_slice(&final_bytes)
            .map_err(|e| StatusError::bad_request().brief(format!("invalid json body: {}", e)))?;

        Ok::<HpkeRequest<T>, StatusError>(HpkeRequest(value))
    }
}
impl<'de, T> ToRequestBody for HpkeRequest<T>
where
    T: Deserialize<'de> + ToSchema,
{
    fn to_request_body(components: &mut Components) -> RequestBody {
        RequestBody::new()
            .description("Extract HPKE json format data from request.")
            .add_content("application/json", Content::new(T::to_schema(components)))
    }
}

impl<'de, T> EndpointArgRegister for HpkeRequest<T>
where
    T: Deserialize<'de> + ToSchema,
{
    fn register(components: &mut Components, operation: &mut Operation, _arg: &str) {
        let request_body = Self::to_request_body(components);
        let _ = <T as ToSchema>::to_schema(components);
        operation.request_body = Some(request_body);
    }
}

pub struct HpkeResponse<T>(pub T);

impl<T> EndpointOutRegister for HpkeResponse<T>
where
    T: EndpointOutRegister,
{
    fn register(components: &mut Components, operation: &mut Operation) {
        T::register(components, operation);
    }
}

#[async_trait]
impl<T> Scribe for HpkeResponse<T>
where
    T: Serialize + Send,
{
    fn render(self, res: &mut Response) {
        let plaintext = match serde_json::to_vec(&self.0) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = ?e, "HpkeJson serialize failed");
                res.render(StatusError::internal_server_error());
                return;
            }
        };

        // try get session pub key from header
        let session_pubkey = res.headers().get_base64("X-Session-PubKey");
        tracing::info!("HPKE[res]: session_pubkey from header: {:?}", session_pubkey);
        let aad = res.headers().get_bytes("X-Path");
        tracing::info!("HPKE[res]: aad from X-Path header: {:?}", aad);

        let (Some(session_pubkey), Some(aad)) = (session_pubkey, aad) else {
            tracing::info!("HPKE[res]: no HPKE response key found, treat as plain JSON");
            res.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static("application/json; charset=utf-8"),
            );
            let _ = res.write_body(plaintext);
            return;
        };

        tracing::info!("HPKE[res]: HPKE headers found, encrypting response...");
        let (encapped_key, ciphertext) = match hpke::encrypt_data(&plaintext, &session_pubkey, &aad) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = ?e, "HpkeJson encrypt failed");
                res.render(StatusError::internal_server_error());
                return;
            }
        };

        res.headers_mut().set_base64("X-Enc", &encapped_key);
        res.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));

        res.replace_body(ciphertext.into());
    }
}

// define a header helper trait
trait HeaderExt {
    fn get_bytes(&self, name: impl AsRef<str>) -> Option<Vec<u8>>;
    // fn set_bytes(&mut self, name: &'static str, value: &[u8]);
    fn get_base64(&self, name: impl AsRef<str>) -> Option<Vec<u8>>;
    fn set_base64(&mut self, name: &'static str, value: &[u8]);
}

impl HeaderExt for salvo::http::HeaderMap {
    fn get_bytes(&self, name: impl AsRef<str>) -> Option<Vec<u8>> {
        self.get(name.as_ref())
            .and_then(|v| v.to_str().ok())
            .map(|s| s.as_bytes().to_vec())
    }
    // fn set_bytes(&mut self, name: &'static str, value: &[u8]) {
    //     if let Ok(hv) = HeaderValue::from_bytes(value) {
    //         self.insert(name, hv);
    //     }
    // }
    fn get_base64(&self, name: impl AsRef<str>) -> Option<Vec<u8>> {
        self.get(name.as_ref())
            .and_then(|v| v.to_str().ok())
            .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
    }

    fn set_base64(&mut self, name: &'static str, value: &[u8]) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(value);
        if let Ok(hv) = HeaderValue::from_str(&b64) {
            self.insert(name, hv);
        }
    }
}
