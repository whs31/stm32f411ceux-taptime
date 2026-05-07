use tonic::{Request, Status};
use uuid::Uuid;

use crate::jwt;

/// Authenticated user identity injected into request extensions by [`AuthInterceptor`].
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedUser(pub Uuid);

/// gRPC interceptor that validates a Bearer JWT and injects [`AuthenticatedUser`] into
/// request extensions. Attach it to services that require authentication.
#[derive(Clone)]
pub struct AuthInterceptor {
  jwt_secret: String,
}

impl AuthInterceptor {
  pub fn new(jwt_secret: impl Into<String>) -> Self {
    Self {
      jwt_secret: jwt_secret.into(),
    }
  }
}

impl tonic::service::Interceptor for AuthInterceptor {
  fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
    let token = extract_bearer(&request)?;
    let user_id = jwt::verify(&token, &self.jwt_secret)?;
    request.extensions_mut().insert(AuthenticatedUser(user_id));
    Ok(request)
  }
}

fn extract_bearer<T>(request: &Request<T>) -> Result<String, Status> {
  request
    .metadata()
    .get("authorization")
    .ok_or_else(|| Status::unauthenticated("Missing authorization header"))?
    .to_str()
    .map_err(|_| Status::unauthenticated("Invalid authorization header encoding"))?
    .strip_prefix("Bearer ")
    .map(str::to_owned)
    .ok_or_else(|| Status::unauthenticated("Expected Bearer token"))
}
