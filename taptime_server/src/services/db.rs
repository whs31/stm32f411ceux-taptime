use tonic::Status;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct CredRow {
  pub user_id: Uuid,
  pub password_hash: String,
}

#[derive(sqlx::FromRow)]
pub(super) struct UserRow {
  pub id: Uuid,
  pub name: String,
  pub email: String,
  pub organization: Option<String>,
  pub time_zone: String,
  pub created_at: chrono::DateTime<chrono::Utc>,
  pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
  pub rfid_uid: Option<Vec<u8>>,
  pub required_work_hours_secs: i64,
  pub lunch_break_duration_secs: i64,
  pub weekends: Vec<i32>,
  pub remote_days: Vec<i32>,
}

pub(super) fn weekday_from_iso(n: i32) -> chrono::Weekday {
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

pub(super) fn weekday_to_iso(w: chrono::Weekday) -> i32 {
  w.num_days_from_monday() as i32 + 1
}

pub(super) fn row_to_core_user(row: UserRow) -> Result<taptime_core::User, Status> {
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

pub(super) async fn fetch_core_user(
  db: &sqlx::PgPool,
  user_id: Uuid,
) -> Result<taptime_core::User, Status> {
  let row = sqlx::query_as::<_, UserRow>(include_str!("queries/fetch_user.sql"))
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?
    .ok_or_else(|| Status::not_found("User not found"))?;

  row_to_core_user(row)
}
