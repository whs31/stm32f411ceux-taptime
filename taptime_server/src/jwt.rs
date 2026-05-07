use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use tonic::Status;
use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Claims {
  pub sub: String,
  pub iat: u64,
  pub exp: u64,
}

pub fn sign(user_id: Uuid, secret: &str) -> Result<String, Status> {
  let now = chrono::Utc::now();
  let claims = Claims {
    sub: user_id.to_string(),
    iat: now.timestamp() as u64,
    exp: (now + chrono::Duration::days(30)).timestamp() as u64,
  };
  encode(
    &Header::default(),
    &claims,
    &EncodingKey::from_secret(secret.as_bytes()),
  )
  .map_err(|e| Status::internal(format!("JWT sign error: {e}")))
}

pub fn verify(token: &str, secret: &str) -> Result<Uuid, Status> {
  decode::<Claims>(
    token,
    &DecodingKey::from_secret(secret.as_bytes()),
    &Validation::default(),
  )
  .map_err(|_| Status::unauthenticated("Invalid or expired token"))
  .and_then(|d| Uuid::parse_str(&d.claims.sub).map_err(|_| Status::internal("Malformed token")))
}
