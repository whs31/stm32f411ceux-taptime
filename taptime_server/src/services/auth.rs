use std::sync::Arc;

use argon2::{
  Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
  password_hash::{SaltString, rand_core::OsRng},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use taptime_schema::{
  User,
  services::{AuthResponse, LoginRequest, RegisterUserRequest, auth_service_server::AuthService},
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub struct AuthServiceImpl {
  db: sqlx::PgPool,
  jwt_secret: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Claims {
  sub: String,
  iat: u64,
  exp: u64,
}

#[derive(sqlx::FromRow)]
struct UserRow {
  id: Uuid,
  name: String,
  email: String,
  organization: Option<String>,
  time_zone: String,
  created_at: chrono::DateTime<chrono::Utc>,
  last_seen: Option<chrono::DateTime<chrono::Utc>>,
  rfid_uid: Option<Vec<u8>>,
  required_work_hours_secs: i64,
  lunch_break_duration_secs: i64,
  weekends: Vec<i32>,
  remote_days: Vec<i32>,
}

#[derive(sqlx::FromRow)]
struct CredRow {
  user_id: Uuid,
  password_hash: String,
}

impl AuthServiceImpl {
  pub fn new(db: sqlx::PgPool, jwt_secret: String) -> Self {
    Self { db, jwt_secret }
  }

  fn make_jwt(&self, user_id: Uuid) -> Result<String, Status> {
    let now = chrono::Utc::now();
    let claims = Claims {
      sub: user_id.to_string(),
      iat: now.timestamp() as u64,
      exp: (now + chrono::Duration::days(30)).timestamp() as u64,
    };
    encode(
      &Header::default(),
      &claims,
      &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
    )
    .map_err(|e| Status::internal(format!("JWT sign error: {e}")))
  }

  fn decode_jwt(&self, token: &str) -> Result<Uuid, Status> {
    decode::<Claims>(
      token,
      &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
      &Validation::default(),
    )
    .map_err(|_| Status::unauthenticated("Invalid or expired token"))
    .and_then(|d| Uuid::parse_str(&d.claims.sub).map_err(|_| Status::internal("Malformed token")))
  }

  async fn fetch_user(&self, id: Uuid) -> Result<taptime_core::User, Status> {
    let row = sqlx::query_as::<_, UserRow>(include_str!("queries/fetch_user.sql"))
      .bind(id)
      .fetch_optional(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?
      .ok_or_else(|| Status::not_found("User not found"))?;

    row_to_core_user(row)
  }
}

fn row_to_core_user(row: UserRow) -> Result<taptime_core::User, Status> {
  let time_zone = row
    .time_zone
    .parse::<chrono_tz::Tz>()
    .map_err(|_| Status::internal("Invalid timezone stored in database"))?;

  let rfid_uid = row
    .rfid_uid
    .map(|bytes| taptime_core::Uid::try_from(taptime_schema::Uid { value: bytes }))
    .transpose()
    .map_err(|_| Status::internal("Invalid RFID UID stored in database"))?;

  Ok(taptime_core::User {
    id: row.id,
    name: row.name,
    email: row.email,
    organization: row.organization,
    time_zone,
    created_at: row.created_at,
    last_seen: row.last_seen,
    rfid_uid,
    settings: taptime_core::UserSettings {
      required_work_hours: chrono::Duration::seconds(row.required_work_hours_secs),
      lunch_break_duration: chrono::Duration::seconds(row.lunch_break_duration_secs),
      weekends: row.weekends.into_iter().map(weekday_from_iso).collect(),
      remote_days: row.remote_days.into_iter().map(weekday_from_iso).collect(),
    },
  })
}

fn weekday_to_iso(w: chrono::Weekday) -> i32 {
  w.num_days_from_monday() as i32 + 1
}

fn weekday_from_iso(n: i32) -> chrono::Weekday {
  match n {
    1 => chrono::Weekday::Mon,
    2 => chrono::Weekday::Tue,
    3 => chrono::Weekday::Wed,
    4 => chrono::Weekday::Thu,
    5 => chrono::Weekday::Fri,
    6 => chrono::Weekday::Sat,
    _ => chrono::Weekday::Sun,
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

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
  async fn register_user(
    self: Arc<Self>,
    request: Request<RegisterUserRequest>,
  ) -> Result<Response<AuthResponse>, Status> {
    let req = request.into_inner();
    let proto_user = req
      .user
      .ok_or_else(|| Status::invalid_argument("Missing user"))?;

    if req.password.is_empty() {
      return Err(Status::invalid_argument("Password cannot be empty"));
    }
    if proto_user.name.is_empty() || proto_user.email.is_empty() {
      return Err(Status::invalid_argument("Name and email are required"));
    }

    let time_zone: chrono_tz::Tz = proto_user
      .time_zone
      .ok_or_else(|| Status::invalid_argument("Missing time_zone"))?
      .try_into()
      .map_err(|e: taptime_schema::Error| Status::invalid_argument(e.to_string()))?;

    let settings = proto_user
      .settings
      .as_ref()
      .map(taptime_core::UserSettings::try_from)
      .transpose()
      .map_err(|e: taptime_schema::Error| Status::invalid_argument(e.to_string()))?
      .unwrap_or_default();

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
      .bind(&proto_user.email)
      .fetch_one(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    if exists {
      return Err(Status::already_exists("Email already registered"));
    }

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
      .hash_password(req.password.as_bytes(), &salt)
      .map_err(|e| Status::internal(format!("Hashing failed: {e}")))?
      .to_string();

    let user_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let weekends: Vec<i32> = settings
      .weekends
      .iter()
      .copied()
      .map(weekday_to_iso)
      .collect();
    let remote_days: Vec<i32> = settings
      .remote_days
      .iter()
      .copied()
      .map(weekday_to_iso)
      .collect();

    sqlx::query(include_str!("queries/create_user.sql"))
      .bind(user_id)
      .bind(&proto_user.name)
      .bind(&proto_user.email)
      .bind(&proto_user.organization)
      .bind(time_zone.name())
      .bind(now)
      .bind(now)
      .bind(None::<Vec<u8>>)
      .bind(settings.required_work_hours.num_seconds())
      .bind(settings.lunch_break_duration.num_seconds())
      .bind(&weekends)
      .bind(&remote_days)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;

    sqlx::query(include_str!("queries/write_user_credentials.sql"))
      .bind(user_id)
      .bind(&proto_user.email)
      .bind(&password_hash)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;

    tracing::info!(user_id = %user_id, email = %proto_user.email, "user registered");

    let user = self.fetch_user(user_id).await?;
    let jwt = self.make_jwt(user_id)?;
    Ok(Response::new(AuthResponse {
      jwt,
      user: Some(User::from(&user)),
    }))
  }

  async fn login(
    self: Arc<Self>,
    request: Request<LoginRequest>,
  ) -> Result<Response<AuthResponse>, Status> {
    let req = request.into_inner();

    let cred = sqlx::query_as::<_, CredRow>(
      include_str!("queries/fetch_user_credentials.sql"),
    )
    .bind(&req.email)
    .fetch_optional(&self.db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?
    .ok_or_else(|| Status::not_found("User not found"))?;

    let parsed_hash =
      PasswordHash::new(&cred.password_hash).map_err(|e| Status::internal(e.to_string()))?;
    Argon2::default()
      .verify_password(req.password.as_bytes(), &parsed_hash)
      .map_err(|_| Status::unauthenticated("Invalid password"))?;

    sqlx::query(include_str!("queries/update_user_last_seen.sql"))
      .bind(cred.user_id)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;

    tracing::info!(user_id = %cred.user_id, email = %req.email, "user logged in");

    let user = self.fetch_user(cred.user_id).await?;
    let jwt = self.make_jwt(cred.user_id)?;
    Ok(Response::new(AuthResponse {
      jwt,
      user: Some(User::from(&user)),
    }))
  }

  async fn get_user(self: Arc<Self>, request: Request<()>) -> Result<Response<User>, Status> {
    let token = extract_bearer(&request)?;
    let user_id = self.decode_jwt(&token)?;
    tracing::debug!(user_id = %user_id, "fetching authenticated user");
    let user = self.fetch_user(user_id).await?;
    Ok(Response::new(User::from(&user)))
  }
}
