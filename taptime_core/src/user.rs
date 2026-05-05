use crate::Uid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct User {
  pub id: uuid::Uuid,
  pub name: String,
  pub email: String,
  pub organization: Option<String>,
  pub time_zone: chrono_tz::Tz,

  pub created_at: chrono::DateTime<chrono::Utc>,
  pub last_seen: Option<chrono::DateTime<chrono::Utc>>,

  pub rfid_uid: Option<Uid>,
  pub settings: UserSettings,
}

#[derive(Debug, Clone)]
pub struct UserCredentials {
  pub user_id: uuid::Uuid,
  pub email: String,
  pub password_hash: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserSettings {
  pub required_work_hours: chrono::Duration,
  pub lunch_break_duration: chrono::Duration,
  pub weekends: Vec<chrono::Weekday>,
  pub remote_days: Vec<chrono::Weekday>,
}

const DEFAULT_REQUIRED_WORK_HOURS: chrono::Duration = chrono::Duration::hours(8);
const DEFAULT_LUNCH_BREAK_DURATION: chrono::Duration = chrono::Duration::minutes(30);

impl Default for UserSettings {
  fn default() -> Self {
    Self {
      required_work_hours: DEFAULT_REQUIRED_WORK_HOURS,
      lunch_break_duration: DEFAULT_LUNCH_BREAK_DURATION,
      weekends: vec![chrono::Weekday::Sat, chrono::Weekday::Sun],
      remote_days: vec![],
    }
  }
}

impl User {}
