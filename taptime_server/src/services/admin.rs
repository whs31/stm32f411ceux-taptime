use std::sync::Arc;

use argon2::PasswordHash;
use chrono::{TimeZone, Utc};
use taptime_schema::{
  User,
  services::{
    AdminLoginRequest, AdminLoginResponse, AdminUserDetail, AdminUserListItem, AdminUserStats,
    BanKind, BanRecord, CreateBanRequest, DeleteUserRequest, GetUserDetailRequest,
    GetUserStatsRequest, KnownIpAddress, ListBansRequest, ListBansResponse, ListUsersRequest,
    ListUsersResponse, RevokeBanRequest, admin_service_server::AdminService,
  },
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use super::{
  access::normalize_ip_cidr,
  auth::verify_password,
  db::{UserRow, fetch_core_user, row_to_core_user},
  store::StoreServiceImpl,
};

#[derive(sqlx::FromRow)]
struct AdminUserRow {
  id: Uuid,
  name: String,
  email: String,
  organization: Option<String>,
  time_zone: String,
  created_at: chrono::DateTime<Utc>,
  last_seen: Option<chrono::DateTime<Utc>>,
  rfid_uid: Option<Vec<u8>>,
  required_work_hours_secs: i64,
  lunch_break_duration_secs: i64,
  weekends: Vec<i32>,
  remote_days: Vec<i32>,
  start_date_days: Option<i32>,
  user_banned: bool,
  known_ip_count: i64,
}

#[derive(sqlx::FromRow)]
struct UserBanRow {
  id: Uuid,
  user_id: Uuid,
  reason: String,
  created_at: chrono::DateTime<Utc>,
  expires_at: Option<chrono::DateTime<Utc>>,
  revoked_at: Option<chrono::DateTime<Utc>>,
  active: bool,
}

#[derive(sqlx::FromRow)]
struct IpBanRow {
  id: Uuid,
  ip_cidr: String,
  reason: String,
  created_at: chrono::DateTime<Utc>,
  expires_at: Option<chrono::DateTime<Utc>>,
  revoked_at: Option<chrono::DateTime<Utc>>,
  active: bool,
}

#[derive(sqlx::FromRow)]
struct KnownIpRow {
  ip: String,
  first_seen: chrono::DateTime<Utc>,
  last_seen: chrono::DateTime<Utc>,
  request_count: i64,
}

pub struct AdminServiceImpl {
  db: sqlx::PgPool,
  jwt_secret: String,
  admin_password_hash: Option<String>,
  token_ttl: chrono::Duration,
  store: StoreServiceImpl,
}

impl AdminServiceImpl {
  pub fn new(
    db: sqlx::PgPool,
    jwt_secret: String,
    admin_password_hash: Option<String>,
    token_ttl: chrono::Duration,
    store: StoreServiceImpl,
  ) -> Self {
    let admin_password_hash = admin_password_hash
      .map(|hash| hash.trim().to_string())
      .filter(|hash| !hash.is_empty());
    Self {
      db,
      jwt_secret,
      admin_password_hash,
      token_ttl,
      store,
    }
  }

  fn require_admin<T>(&self, request: &Request<T>) -> Result<(), Status> {
    let token = extract_bearer(request)?;
    crate::jwt::verify_admin(&token, &self.jwt_secret)
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

fn user_row(row: AdminUserRow) -> UserRow {
  UserRow {
    id: row.id,
    name: row.name,
    email: row.email,
    organization: row.organization,
    time_zone: row.time_zone,
    created_at: row.created_at,
    last_seen: row.last_seen,
    rfid_uid: row.rfid_uid,
    required_work_hours_secs: row.required_work_hours_secs,
    lunch_break_duration_secs: row.lunch_break_duration_secs,
    weekends: row.weekends,
    remote_days: row.remote_days,
    start_date_days: row.start_date_days,
  }
}

fn timestamp(value: chrono::DateTime<Utc>) -> prost_types::Timestamp {
  prost_types::Timestamp {
    seconds: value.timestamp(),
    nanos: value.timestamp_subsec_nanos() as i32,
  }
}

fn stats_dates(
  time_zone: chrono_tz::Tz,
  configured_start: Option<chrono::NaiveDate>,
  created_at: chrono::DateTime<Utc>,
  generated_at: chrono::DateTime<Utc>,
) -> (chrono::NaiveDate, chrono::NaiveDate) {
  let today = generated_at.with_timezone(&time_zone).date_naive();
  let created_date = created_at.with_timezone(&time_zone).date_naive();
  (today, configured_start.unwrap_or(created_date))
}

fn datetime(
  value: prost_types::Timestamp,
  field: &'static str,
) -> Result<chrono::DateTime<Utc>, Status> {
  Utc
    .timestamp_opt(value.seconds, value.nanos as u32)
    .single()
    .ok_or_else(|| Status::invalid_argument(format!("Invalid {field}")))
}

fn optional_expiry(
  value: Option<prost_types::Timestamp>,
) -> Result<Option<chrono::DateTime<Utc>>, Status> {
  let expiry = value.map(|ts| datetime(ts, "expires_at")).transpose()?;
  if expiry.is_some_and(|expires_at| expires_at <= Utc::now()) {
    return Err(Status::invalid_argument("expires_at must be in the future"));
  }
  Ok(expiry)
}

fn user_ban_record(row: UserBanRow) -> BanRecord {
  BanRecord {
    id: Some(row.id.into()),
    kind: BanKind::User as i32,
    user_id: Some(row.user_id.into()),
    ip_cidr: String::new(),
    reason: row.reason,
    created_at: Some(timestamp(row.created_at)),
    expires_at: row.expires_at.map(timestamp),
    revoked_at: row.revoked_at.map(timestamp),
    active: row.active,
  }
}

fn ip_ban_record(row: IpBanRow) -> BanRecord {
  BanRecord {
    id: Some(row.id.into()),
    kind: BanKind::Ip as i32,
    user_id: None,
    ip_cidr: row.ip_cidr,
    reason: row.reason,
    created_at: Some(timestamp(row.created_at)),
    expires_at: row.expires_at.map(timestamp),
    revoked_at: row.revoked_at.map(timestamp),
    active: row.active,
  }
}

fn normalize_reason(reason: String) -> String {
  reason.trim().to_string()
}

#[tonic::async_trait]
impl AdminService for AdminServiceImpl {
  async fn admin_login(
    self: Arc<Self>,
    request: Request<AdminLoginRequest>,
  ) -> Result<Response<AdminLoginResponse>, Status> {
    let Some(password_hash) = &self.admin_password_hash else {
      return Err(Status::failed_precondition("Admin access is disabled"));
    };
    PasswordHash::new(password_hash).map_err(|_| {
      Status::failed_precondition(
        "ADMIN_PASSWORD_HASH is not a valid Argon2 PHC string; if using Docker Compose, wrap the hash in single quotes so $ fields are not interpolated",
      )
    })?;
    let req = request.into_inner();
    verify_password(&req.password, password_hash)?;
    let (token, expires_at) = crate::jwt::sign_admin(&self.jwt_secret, self.token_ttl)?;
    Ok(Response::new(AdminLoginResponse {
      admin_token: token,
      expires_at: Some(timestamp(expires_at)),
    }))
  }

  async fn list_users(
    self: Arc<Self>,
    request: Request<ListUsersRequest>,
  ) -> Result<Response<ListUsersResponse>, Status> {
    self.require_admin(&request)?;
    let req = request.into_inner();
    let query = req.query.trim().to_string();
    let pattern = format!("%{query}%");
    let limit = req.limit.clamp(1, 500);
    let offset = req.offset.max(0);

    let total: i64 = sqlx::query_scalar(
      "SELECT COUNT(*) FROM users
       WHERE ($1 = '' OR name ILIKE $2 OR email ILIKE $2 OR COALESCE(organization, '') ILIKE $2)",
    )
    .bind(&query)
    .bind(&pattern)
    .fetch_one(&self.db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?;

    let rows = sqlx::query_as::<_, AdminUserRow>(
      "SELECT id, name, email, organization, time_zone, created_at, last_seen, rfid_uid,
              required_work_hours_secs, lunch_break_duration_secs, weekends, remote_days,
              start_date_days,
              EXISTS(
                SELECT 1 FROM user_bans
                WHERE user_bans.user_id = users.id
                  AND revoked_at IS NULL
                  AND (expires_at IS NULL OR expires_at > NOW())
              ) AS user_banned,
              COALESCE((
                SELECT COUNT(*) FROM user_ip_addresses
                WHERE user_ip_addresses.user_id = users.id
              ), 0)::BIGINT AS known_ip_count
       FROM users
       WHERE ($1 = '' OR name ILIKE $2 OR email ILIKE $2 OR COALESCE(organization, '') ILIKE $2)
       ORDER BY created_at DESC
       LIMIT $3 OFFSET $4",
    )
    .bind(&query)
    .bind(&pattern)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(&self.db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?;

    let users = rows
      .into_iter()
      .map(|row| {
        let user_banned = row.user_banned;
        let known_ip_count = row.known_ip_count;
        row_to_core_user(user_row(row)).map(|user| AdminUserListItem {
          user: Some(User::from(&user)),
          user_banned,
          known_ip_count,
        })
      })
      .collect::<Result<Vec<_>, _>>()?;

    Ok(Response::new(ListUsersResponse { users, total }))
  }

  async fn get_user_detail(
    self: Arc<Self>,
    request: Request<GetUserDetailRequest>,
  ) -> Result<Response<AdminUserDetail>, Status> {
    self.require_admin(&request)?;
    let user_id: Uuid = request
      .into_inner()
      .user_id
      .ok_or_else(|| Status::invalid_argument("Missing user_id"))?
      .into();
    let user = fetch_core_user(&self.db, user_id).await?;

    let known_ip_rows = sqlx::query_as::<_, KnownIpRow>(
      "SELECT ip, first_seen, last_seen, request_count
       FROM user_ip_addresses
       WHERE user_id = $1
       ORDER BY last_seen DESC",
    )
    .bind(user_id)
    .fetch_all(&self.db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?;
    let known_ips = known_ip_rows
      .into_iter()
      .map(|row| KnownIpAddress {
        ip: row.ip,
        first_seen: Some(timestamp(row.first_seen)),
        last_seen: Some(timestamp(row.last_seen)),
        request_count: row.request_count,
      })
      .collect();

    let user_bans = sqlx::query_as::<_, UserBanRow>(
      "SELECT id, user_id, reason, created_at, expires_at, revoked_at,
              (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())) AS active
       FROM user_bans
       WHERE user_id = $1
       ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&self.db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?;
    let bans = user_bans.into_iter().map(user_ban_record).collect();

    let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE user_id = $1")
      .bind(user_id)
      .fetch_one(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    let day_flag_count: i64 =
      sqlx::query_scalar("SELECT COUNT(*) FROM day_flags WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(AdminUserDetail {
      user: Some(User::from(&user)),
      known_ips,
      bans,
      event_count,
      day_flag_count,
    }))
  }

  async fn get_user_stats(
    self: Arc<Self>,
    request: Request<GetUserStatsRequest>,
  ) -> Result<Response<AdminUserStats>, Status> {
    self.require_admin(&request)?;
    let user_id: Uuid = request
      .into_inner()
      .user_id
      .ok_or_else(|| Status::invalid_argument("Missing user_id"))?
      .into();
    let user = fetch_core_user(&self.db, user_id).await?;
    let generated_at = Utc::now();
    let (today, overall_start) = stats_dates(
      user.time_zone,
      user.settings.start_date,
      user.created_at,
      generated_at,
    );
    let (today_summary, month_to_date, overall) = self
      .store
      .build_user_stats(user_id, overall_start, today)
      .await?;

    Ok(Response::new(AdminUserStats {
      today: Some(today.into()),
      overall_start: Some(overall_start.into()),
      generated_at: Some(timestamp(generated_at)),
      today_summary: Some(today_summary),
      month_to_date: Some(month_to_date),
      overall: Some(overall),
    }))
  }

  async fn list_bans(
    self: Arc<Self>,
    request: Request<ListBansRequest>,
  ) -> Result<Response<ListBansResponse>, Status> {
    self.require_admin(&request)?;
    let req = request.into_inner();
    let kind = BanKind::try_from(req.kind).unwrap_or(BanKind::Unspecified);
    let include_inactive = req.include_inactive;
    let mut bans = Vec::new();

    if matches!(kind, BanKind::Unspecified | BanKind::User) {
      let rows = sqlx::query_as::<_, UserBanRow>(
        "SELECT id, user_id, reason, created_at, expires_at, revoked_at,
                (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())) AS active
         FROM user_bans
         WHERE ($1 OR (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())))
         ORDER BY created_at DESC",
      )
      .bind(include_inactive)
      .fetch_all(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
      bans.extend(rows.into_iter().map(user_ban_record));
    }
    if matches!(kind, BanKind::Unspecified | BanKind::Ip) {
      let rows = sqlx::query_as::<_, IpBanRow>(
        "SELECT id, ip_cidr, reason, created_at, expires_at, revoked_at,
                (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())) AS active
         FROM ip_bans
         WHERE ($1 OR (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())))
         ORDER BY created_at DESC",
      )
      .bind(include_inactive)
      .fetch_all(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
      bans.extend(rows.into_iter().map(ip_ban_record));
    }
    bans.sort_by(|a, b| {
      b.created_at
        .as_ref()
        .map(|ts| ts.seconds)
        .cmp(&a.created_at.as_ref().map(|ts| ts.seconds))
    });

    Ok(Response::new(ListBansResponse { bans }))
  }

  async fn create_ban(
    self: Arc<Self>,
    request: Request<CreateBanRequest>,
  ) -> Result<Response<BanRecord>, Status> {
    self.require_admin(&request)?;
    let req = request.into_inner();
    let reason = normalize_reason(req.reason);
    let expires_at = optional_expiry(req.expires_at)?;
    let kind = BanKind::try_from(req.kind).unwrap_or(BanKind::Unspecified);

    match kind {
      BanKind::User => {
        let user_id: Uuid = req
          .user_id
          .ok_or_else(|| Status::invalid_argument("Missing user_id"))?
          .into();
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
          .bind(user_id)
          .fetch_one(&self.db)
          .await
          .map_err(|e| Status::internal(e.to_string()))?;
        if !exists {
          return Err(Status::not_found("User not found"));
        }
        let row = sqlx::query_as::<_, UserBanRow>(
          "INSERT INTO user_bans (user_id, reason, expires_at)
           VALUES ($1, $2, $3)
           RETURNING id, user_id, reason, created_at, expires_at, revoked_at,
                     (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())) AS active",
        )
        .bind(user_id)
        .bind(reason)
        .bind(expires_at)
        .fetch_one(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(user_ban_record(row)))
      }
      BanKind::Ip => {
        let ip_cidr = normalize_ip_cidr(&req.ip_cidr)?;
        let row = sqlx::query_as::<_, IpBanRow>(
          "INSERT INTO ip_bans (ip_cidr, reason, expires_at)
           VALUES ($1, $2, $3)
           RETURNING id, ip_cidr, reason, created_at, expires_at, revoked_at,
                     (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())) AS active",
        )
        .bind(ip_cidr)
        .bind(reason)
        .bind(expires_at)
        .fetch_one(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(ip_ban_record(row)))
      }
      BanKind::Unspecified => Err(Status::invalid_argument("Missing ban kind")),
    }
  }

  async fn revoke_ban(
    self: Arc<Self>,
    request: Request<RevokeBanRequest>,
  ) -> Result<Response<BanRecord>, Status> {
    self.require_admin(&request)?;
    let req = request.into_inner();
    let ban_id: Uuid = req
      .ban_id
      .ok_or_else(|| Status::invalid_argument("Missing ban_id"))?
      .into();
    let kind = BanKind::try_from(req.kind).unwrap_or(BanKind::Unspecified);

    match kind {
      BanKind::User => {
        let row = sqlx::query_as::<_, UserBanRow>(
          "UPDATE user_bans
           SET revoked_at = COALESCE(revoked_at, NOW())
           WHERE id = $1
           RETURNING id, user_id, reason, created_at, expires_at, revoked_at,
                     (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())) AS active",
        )
        .bind(ban_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?
        .ok_or_else(|| Status::not_found("Ban not found"))?;
        Ok(Response::new(user_ban_record(row)))
      }
      BanKind::Ip => {
        let row = sqlx::query_as::<_, IpBanRow>(
          "UPDATE ip_bans
           SET revoked_at = COALESCE(revoked_at, NOW())
           WHERE id = $1
           RETURNING id, ip_cidr, reason, created_at, expires_at, revoked_at,
                     (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())) AS active",
        )
        .bind(ban_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?
        .ok_or_else(|| Status::not_found("Ban not found"))?;
        Ok(Response::new(ip_ban_record(row)))
      }
      BanKind::Unspecified => Err(Status::invalid_argument("Missing ban kind")),
    }
  }

  async fn delete_user_time_data(
    self: Arc<Self>,
    request: Request<DeleteUserRequest>,
  ) -> Result<Response<()>, Status> {
    self.require_admin(&request)?;
    let user_id: Uuid = request
      .into_inner()
      .user_id
      .ok_or_else(|| Status::invalid_argument("Missing user_id"))?
      .into();
    let mut tx = self
      .db
      .begin()
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    sqlx::query(include_str!("queries/delete_events.sql"))
      .bind(user_id)
      .execute(&mut *tx)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    sqlx::query(include_str!("queries/delete_day_flags.sql"))
      .bind(user_id)
      .execute(&mut *tx)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    sqlx::query(include_str!(
      "queries/delete_day_required_work_overrides.sql"
    ))
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| Status::internal(e.to_string()))?;
    tx.commit()
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    Ok(Response::new(()))
  }

  async fn delete_user_account(
    self: Arc<Self>,
    request: Request<DeleteUserRequest>,
  ) -> Result<Response<()>, Status> {
    self.require_admin(&request)?;
    let user_id: Uuid = request
      .into_inner()
      .user_id
      .ok_or_else(|| Status::invalid_argument("Missing user_id"))?
      .into();
    let result = sqlx::query(include_str!("queries/delete_user.sql"))
      .bind(user_id)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    if result.rows_affected() == 0 {
      return Err(Status::not_found("User not found"));
    }
    Ok(Response::new(()))
  }
}

#[cfg(test)]
mod tests {
  use chrono::NaiveDate;

  use super::*;

  #[test]
  fn optional_expiry_rejects_past_timestamp() {
    let past = timestamp(Utc::now() - chrono::Duration::seconds(1));
    assert!(optional_expiry(Some(past)).is_err());
  }

  #[test]
  fn stats_dates_use_user_timezone_and_configured_start() {
    let generated_at = Utc.with_ymd_and_hms(2024, 1, 1, 21, 30, 0).unwrap();
    let created_at = Utc.with_ymd_and_hms(2023, 12, 1, 22, 0, 0).unwrap();
    let configured_start = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

    let (today, overall_start) = stats_dates(
      chrono_tz::Europe::Moscow,
      Some(configured_start),
      created_at,
      generated_at,
    );

    assert_eq!(today, NaiveDate::from_ymd_opt(2024, 1, 2).unwrap());
    assert_eq!(overall_start, configured_start);
  }

  #[test]
  fn stats_dates_fall_back_to_local_account_creation_date() {
    let generated_at = Utc.with_ymd_and_hms(2024, 2, 1, 12, 0, 0).unwrap();
    let created_at = Utc.with_ymd_and_hms(2024, 1, 1, 23, 30, 0).unwrap();

    let (_, overall_start) = stats_dates(chrono_tz::Europe::Moscow, None, created_at, generated_at);

    assert_eq!(overall_start, NaiveDate::from_ymd_opt(2024, 1, 2).unwrap());
  }
}
