use std::sync::Arc;

use chrono::Datelike;
use taptime_core::{DayFlags, LocalTime};
use taptime_schema::{
  Date,
  services::{
    EventRequest, SetFlagRequest,
    store_service_server::StoreService,
  },
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::interceptors::AuthenticatedUser;

pub struct StoreServiceImpl {
  db: sqlx::PgPool,
}

impl StoreServiceImpl {
  pub fn new(db: sqlx::PgPool) -> Self {
    Self { db }
  }
}

#[derive(sqlx::FromRow)]
struct SettingsRow {
  required_work_hours_secs: i64,
  lunch_break_duration_secs: i64,
  weekends: Vec<i32>,
  remote_days: Vec<i32>,
}

#[derive(sqlx::FromRow)]
struct EventRow {
  event_kind: i16,
  hour: i16,
  minute: i16,
  second: i16,
}

fn extract_user<T>(request: &Request<T>) -> Result<Uuid, Status> {
  request
    .extensions()
    .get::<AuthenticatedUser>()
    .map(|u| u.0)
    .ok_or_else(|| Status::unauthenticated("Not authenticated"))
}

fn date_to_epoch_days(date: chrono::NaiveDate) -> i32 {
  taptime_schema::Date::from(date).days_since_epoch
}

impl StoreServiceImpl {
  async fn build_day(
    &self,
    user_id: Uuid,
    date: chrono::NaiveDate,
  ) -> Result<taptime_core::Day, Status> {
    let days = date_to_epoch_days(date);

    let settings =
      sqlx::query_as::<_, SettingsRow>(include_str!("queries/fetch_user_settings.sql"))
        .bind(user_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?
        .ok_or_else(|| Status::not_found("User not found"))?;

    let iso_weekday = date.weekday().num_days_from_monday() as i32 + 1;
    let mut flags = DayFlags::empty();
    if settings.weekends.contains(&iso_weekday) {
      flags.insert(DayFlags::WEEKEND);
    }
    if settings.remote_days.contains(&iso_weekday) {
      flags.insert(DayFlags::REMOTE);
    }

    let stored_flags: Option<i32> =
      sqlx::query_scalar(include_str!("queries/fetch_day_flags.sql"))
        .bind(user_id)
        .bind(days)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    if let Some(f) = stored_flags {
      flags = DayFlags::from_bits_truncate(f as u32);
    }

    let event_rows = sqlx::query_as::<_, EventRow>(include_str!("queries/fetch_events.sql"))
      .bind(user_id)
      .bind(days)
      .fetch_all(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;

    let events = event_rows
      .into_iter()
      .map(|row| {
        let lt = LocalTime::new(row.hour as u32, row.minute as u32, row.second as u32)
          .map_err(|e| Status::internal(e.to_string()))?;
        Ok(match row.event_kind {
          0 => taptime_core::Event::CheckIn(lt),
          _ => taptime_core::Event::CheckOut(lt),
        })
      })
      .collect::<Result<Vec<_>, Status>>()?;

    Ok(taptime_core::Day {
      date,
      events,
      flags,
      required_work_hours: chrono::Duration::seconds(settings.required_work_hours_secs),
      lunch_break_duration: chrono::Duration::seconds(settings.lunch_break_duration_secs),
    })
  }
}

#[tonic::async_trait]
impl StoreService for StoreServiceImpl {
  async fn get_day(
    self: Arc<Self>,
    request: Request<Date>,
  ) -> Result<Response<taptime_schema::Day>, Status> {
    let user_id = extract_user(&request)?;
    let date: chrono::NaiveDate = request.into_inner().into();
    let day = self.build_day(user_id, date).await?;
    Ok(Response::new(taptime_schema::Day::from(&day)))
  }

  async fn get_work_hours(
    self: Arc<Self>,
    request: Request<Date>,
  ) -> Result<Response<prost_types::Duration>, Status> {
    let user_id = extract_user(&request)?;
    let date: chrono::NaiveDate = request.into_inner().into();
    let day = self.build_day(user_id, date).await?;
    let duration = day
      .work_hours()
      .map_err(|e| Status::internal(e.to_string()))?
      .unwrap_or(chrono::Duration::zero());
    Ok(Response::new(duration.into()))
  }

  async fn get_balance(
    self: Arc<Self>,
    request: Request<Date>,
  ) -> Result<Response<taptime_schema::Balance>, Status> {
    let user_id = extract_user(&request)?;
    let date: chrono::NaiveDate = request.into_inner().into();
    let day = self.build_day(user_id, date).await?;
    let balance = day.balance().map_err(|e| Status::internal(e.to_string()))?;
    Ok(Response::new(taptime_schema::Balance::from(&balance)))
  }

  async fn set_flag(
    self: Arc<Self>,
    request: Request<SetFlagRequest>,
  ) -> Result<Response<()>, Status> {
    let user_id = extract_user(&request)?;
    let req = request.into_inner();
    let date: chrono::NaiveDate = req
      .date
      .ok_or_else(|| Status::invalid_argument("Missing date"))?
      .into();
    let days = date_to_epoch_days(date);

    let day = self.build_day(user_id, date).await?;
    let flag_bit = DayFlags::from_bits_truncate(req.flag as u32);
    let new_flags = day.flags ^ flag_bit;

    sqlx::query(include_str!("queries/upsert_day_flags.sql"))
      .bind(user_id)
      .bind(days)
      .bind(new_flags.bits() as i32)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(()))
  }

  async fn add_check_in(
    self: Arc<Self>,
    request: Request<EventRequest>,
  ) -> Result<Response<()>, Status> {
    let user_id = extract_user(&request)?;
    let req = request.into_inner();
    let date: chrono::NaiveDate = req
      .date
      .ok_or_else(|| Status::invalid_argument("Missing date"))?
      .into();
    let proto_time = req
      .time
      .ok_or_else(|| Status::invalid_argument("Missing time"))?;
    let (h, m, s) = (proto_time.hour, proto_time.minute, proto_time.second);
    let lt = LocalTime::new(h, m, s).map_err(|e| Status::invalid_argument(e.to_string()))?;

    let mut day = self.build_day(user_id, date).await?;
    day
      .add_check_in(lt)
      .map_err(|e| Status::failed_precondition(e.to_string()))?;

    let days = date_to_epoch_days(date);
    sqlx::query(include_str!("queries/insert_event.sql"))
      .bind(user_id)
      .bind(days)
      .bind(0i16)
      .bind(h as i16)
      .bind(m as i16)
      .bind(s as i16)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(()))
  }

  async fn add_check_out(
    self: Arc<Self>,
    request: Request<EventRequest>,
  ) -> Result<Response<()>, Status> {
    let user_id = extract_user(&request)?;
    let req = request.into_inner();
    let date: chrono::NaiveDate = req
      .date
      .ok_or_else(|| Status::invalid_argument("Missing date"))?
      .into();
    let proto_time = req
      .time
      .ok_or_else(|| Status::invalid_argument("Missing time"))?;
    let (h, m, s) = (proto_time.hour, proto_time.minute, proto_time.second);
    let lt = LocalTime::new(h, m, s).map_err(|e| Status::invalid_argument(e.to_string()))?;

    let mut day = self.build_day(user_id, date).await?;
    day
      .add_check_out(lt)
      .map_err(|e| Status::failed_precondition(e.to_string()))?;

    let days = date_to_epoch_days(date);
    sqlx::query(include_str!("queries/insert_event.sql"))
      .bind(user_id)
      .bind(days)
      .bind(1i16)
      .bind(h as i16)
      .bind(m as i16)
      .bind(s as i16)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(()))
  }
}
