use chrono::{Datelike, NaiveDate, TimeZone};

use crate::{Day, DayFlags, Error, Uid};

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
  pub id: uuid::Uuid,
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

impl User {
  pub fn new_day(&self, date: NaiveDate) -> Day {
    let weekday = date.weekday();
    let mut flags = DayFlags::empty();
    if self.settings.weekends.contains(&weekday) {
      flags.insert(DayFlags::WEEKEND);
    }
    if self.settings.remote_days.contains(&weekday) {
      flags.insert(DayFlags::REMOTE);
    }
    Day {
      date,
      events: vec![],
      flags,
      required_work_hours: self.settings.required_work_hours,
      lunch_break_duration: self.settings.lunch_break_duration,
    }
  }
}

impl TryFrom<&taptime_schema::user::Settings> for UserSettings {
  type Error = taptime_schema::Error;

  fn try_from(value: &taptime_schema::user::Settings) -> Result<Self, Self::Error> {
    let required_work_hours: chrono::Duration = value
      .required_work_hours
      .map(|d| chrono::Duration::new(d.seconds, d.nanos as u32).unwrap_or_default())
      .ok_or(taptime_schema::Error::MissingField("required_work_hours"))?;
    let lunch_break_duration: chrono::Duration = value
      .lunch_break_duration
      .map(|d| chrono::Duration::new(d.seconds, d.nanos as u32).unwrap_or_default())
      .ok_or(taptime_schema::Error::MissingField("lunch_break_duration"))?;
    Ok(Self {
      required_work_hours,
      lunch_break_duration,
      weekends: value
        .weekends()
        .map(|w| w.try_into())
        .collect::<Result<_, _>>()?,
      remote_days: value
        .remote_days()
        .map(|w| w.try_into())
        .collect::<Result<_, _>>()?,
    })
  }
}

impl From<&UserSettings> for taptime_schema::user::Settings {
  fn from(value: &UserSettings) -> Self {
    Self {
      required_work_hours: Some(value.required_work_hours.into()),
      lunch_break_duration: Some(value.lunch_break_duration.into()),
      weekends: value
        .weekends
        .iter()
        .map(|w| taptime_schema::Weekday::from(*w).into())
        .collect(),
      remote_days: value
        .remote_days
        .iter()
        .map(|w| taptime_schema::Weekday::from(*w).into())
        .collect(),
    }
  }
}

impl From<UserSettings> for taptime_schema::user::Settings {
  fn from(value: UserSettings) -> Self {
    Self::from(&value)
  }
}

impl TryFrom<&taptime_schema::user::Credentials> for UserCredentials {
  type Error = taptime_schema::Error;

  fn try_from(value: &taptime_schema::user::Credentials) -> Result<Self, Self::Error> {
    Ok(Self {
      id: value
        .id
        .ok_or(taptime_schema::Error::MissingField("id"))?
        .into(),
      email: value.email.clone(),
      password_hash: value.password_hash.clone(),
    })
  }
}

impl From<UserCredentials> for taptime_schema::user::Credentials {
  fn from(value: UserCredentials) -> Self {
    Self {
      id: Some(value.id.into()),
      email: value.email,
      password_hash: value.password_hash,
    }
  }
}

impl TryFrom<&taptime_schema::User> for User {
  type Error = Error;

  fn try_from(value: &taptime_schema::User) -> Result<Self, Self::Error> {
    Ok(Self {
      id: value
        .id
        .ok_or(taptime_schema::Error::MissingField("id"))?
        .into(),
      name: value.name.clone(),
      email: value.email.clone(),
      organization: value.organization.clone(),
      time_zone: value
        .time_zone
        .clone()
        .ok_or(taptime_schema::Error::MissingField("time_zone"))?
        .try_into()?,
      created_at: value
        .created_at
        .map(|ts| {
          chrono::Utc
            .timestamp_opt(ts.seconds, ts.nanos as u32)
            .unwrap()
        })
        .unwrap_or_default(),
      last_seen: value.last_seen.map(|ts| {
        chrono::Utc
          .timestamp_opt(ts.seconds, ts.nanos as u32)
          .unwrap()
      }),
      rfid_uid: value
        .rfid_uid
        .as_ref()
        .map(|uid| uid.clone().try_into())
        .transpose()?,
      settings: value
        .settings
        .as_ref()
        .ok_or(taptime_schema::Error::MissingField("settings"))?
        .try_into()?,
    })
  }
}

impl From<&User> for taptime_schema::User {
  fn from(value: &User) -> Self {
    let serialize_ts = |ts: &chrono::DateTime<chrono::Utc>| prost_types::Timestamp {
      seconds: ts.timestamp(),
      nanos: ts.timestamp_subsec_nanos() as i32,
    };
    Self {
      id: Some(value.id.into()),
      name: value.name.clone(),
      email: value.email.clone(),
      organization: value.organization.clone(),
      time_zone: Some(value.time_zone.into()),
      created_at: Some(serialize_ts(&value.created_at)),
      last_seen: value.last_seen.map(|ts| serialize_ts(&ts)),
      rfid_uid: value.rfid_uid.as_ref().map(|uid| uid.clone().into()),
      settings: Some(value.settings.clone().into()),
    }
  }
}
