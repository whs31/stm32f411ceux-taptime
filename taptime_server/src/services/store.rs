use std::{collections::BTreeMap, sync::Arc};

use chrono::{Datelike, Duration, NaiveDate};
use taptime_core::{Balance, Day, DayFlags, Event, LocalTime, User};
use taptime_schema::{
  Date,
  services::{
    DashboardRequest, DashboardResponse, DaySummary, DeleteEventRequest, EventRequest,
    MonthlyStats, SetFlagRequest, SetRequiredWorkHoursOverrideRequest,
    store_service_server::StoreService,
  },
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use super::{
  access::{AccessConfig, client_ip, enforce_ip_allowed, enforce_user_allowed, record_user_ip},
  db::fetch_core_user,
};
use crate::interceptors::AuthenticatedUser;

#[derive(Clone)]
pub struct StoreServiceImpl {
  db: sqlx::PgPool,
  access_config: AccessConfig,
}

impl StoreServiceImpl {
  pub fn new(db: sqlx::PgPool, access_config: AccessConfig) -> Self {
    Self { db, access_config }
  }

  async fn authenticated_user<T>(&self, request: &Request<T>) -> Result<Uuid, Status> {
    let user_id = extract_user(request)?;
    let ip = enforce_ip_allowed(&self.db, client_ip(request, self.access_config)).await?;
    enforce_user_allowed(&self.db, user_id).await?;
    record_user_ip(&self.db, user_id, ip).await?;
    Ok(user_id)
  }
}

#[derive(sqlx::FromRow)]
struct EventRangeRow {
  date_days: i32,
  id: Uuid,
  event_kind: i16,
  hour: i16,
  minute: i16,
  second: i16,
}

#[derive(sqlx::FromRow)]
struct DayFlagRangeRow {
  date_days: i32,
  flags: i32,
}

#[derive(sqlx::FromRow)]
struct DayRequiredWorkOverrideRangeRow {
  date_days: i32,
  required_work_hours_secs: i64,
}

#[derive(Debug, Clone)]
struct BuiltDay {
  day: Day,
  event_ids: Vec<Uuid>,
  before_start_date: bool,
  work_target: Duration,
  required_work_hours_overridden: bool,
}

#[derive(Debug, Clone)]
struct StoredEvent {
  id: Uuid,
  event: Event,
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

fn date_from_request(date: Option<Date>, name: &'static str) -> Result<NaiveDate, Status> {
  Ok(
    date
      .ok_or_else(|| Status::invalid_argument(format!("Missing {name}")))?
      .into(),
  )
}

fn next_date(date: NaiveDate) -> Result<NaiveDate, Status> {
  date
    .succ_opt()
    .ok_or_else(|| Status::invalid_argument("Date range overflow"))
}

fn proto_duration_to_chrono(
  duration: prost_types::Duration,
  name: &'static str,
) -> Result<Duration, Status> {
  if duration.nanos < 0 || duration.nanos >= 1_000_000_000 {
    return Err(Status::invalid_argument(format!("Invalid {name}")));
  }
  let duration =
    Duration::seconds(duration.seconds) + Duration::nanoseconds(i64::from(duration.nanos));
  if duration <= Duration::zero() {
    return Err(Status::invalid_argument(format!("{name} must be positive")));
  }
  Ok(duration)
}

fn effective_work_target(user: &User, override_duration: Option<Duration>) -> Duration {
  override_duration.unwrap_or(user.settings.required_work_hours)
}

fn required_work_hours_for(flags: DayFlags, work_target: Duration) -> Duration {
  if flags.intersects(DayFlags::WEEKEND | DayFlags::DAY_OFF | DayFlags::VACATION) {
    Duration::zero()
  } else {
    work_target
  }
}

#[cfg(test)]
fn apply_day_flags(day: &mut Day, flags: DayFlags, user: &User) {
  apply_day_configuration(day, flags, user, None);
}

fn apply_day_configuration(
  day: &mut Day,
  flags: DayFlags,
  user: &User,
  override_duration: Option<Duration>,
) -> Duration {
  let work_target = effective_work_target(user, override_duration);
  day.flags = flags;
  day.required_work_hours = required_work_hours_for(flags, work_target);
  work_target
}

fn row_to_event(
  id: Uuid,
  event_kind: i16,
  hour: i16,
  minute: i16,
  second: i16,
) -> Result<StoredEvent, Status> {
  let lt = LocalTime::new(hour as u32, minute as u32, second as u32)
    .map_err(|e| Status::internal(e.to_string()))?;
  let event = match event_kind {
    0 => Event::CheckIn(lt),
    _ => Event::CheckOut(lt),
  };
  Ok(StoredEvent { id, event })
}

fn is_non_required_day(day: &Day) -> bool {
  day
    .flags
    .intersects(DayFlags::WEEKEND | DayFlags::DAY_OFF | DayFlags::VACATION)
}

fn is_calendar_regular_required_day(day: &Day) -> bool {
  !day
    .flags
    .intersects(DayFlags::WEEKEND | DayFlags::DAY_OFF | DayFlags::VACATION | DayFlags::REMOTE)
}

fn dashboard_balance(day: &Day, before_start_date: bool, skipped: bool) -> Balance {
  if before_start_date || is_non_required_day(day) || day.is_remote() {
    Balance::Exact
  } else {
    let required = if skipped {
      day.required_work_hours
    } else {
      day.required_day_duration()
    };
    Balance::calculate(
      day.presence_duration().unwrap_or(Duration::zero()),
      required,
    )
  }
}

fn is_skipped_day(day: &Day, today: NaiveDate, clocked_work: Duration) -> bool {
  day.date < today
    && is_calendar_regular_required_day(day)
    && day.required_work_hours > Duration::zero()
    && clocked_work <= Duration::zero()
    && day.events.is_empty()
}

fn summarize_day(built_day: &BuiltDay, today: NaiveDate) -> DaySummary {
  let day = &built_day.day;
  let clocked_work = day.clocked_work_duration();
  let skipped = !built_day.before_start_date && is_skipped_day(day, today, clocked_work);
  let balance = dashboard_balance(day, built_day.before_start_date, skipped);
  DaySummary {
    day: Some(schema_day_from_built_day(built_day)),
    clocked_work: Some(clocked_work.into()),
    balance: Some(taptime_schema::Balance::from(&balance)),
    skipped,
    full_day_worked: !built_day.before_start_date && day.full_day_worked(built_day.work_target),
    required_work_hours_overridden: built_day.required_work_hours_overridden,
    work_target: Some(built_day.work_target.into()),
    before_start_date: built_day.before_start_date,
  }
}

fn schema_day_from_built_day(built_day: &BuiltDay) -> taptime_schema::Day {
  let mut day = taptime_schema::Day::from(&built_day.day);
  for (event, id) in day.events.iter_mut().zip(&built_day.event_ids) {
    event.id = Some(taptime_schema::Uuid::from(id));
  }
  day
}

fn build_days_from_maps(
  user: &User,
  start: NaiveDate,
  end: NaiveDate,
  flags_by_day: &BTreeMap<i32, DayFlags>,
  overrides_by_day: &BTreeMap<i32, Duration>,
  mut events_by_day: BTreeMap<i32, Vec<StoredEvent>>,
) -> Result<Vec<BuiltDay>, Status> {
  let mut days = Vec::new();
  let mut date = start;
  loop {
    let days_since_epoch = date_to_epoch_days(date);
    let before_start_date = user.is_before_start_date(date);
    let mut day = user.new_day(date);
    let flags = flags_by_day
      .get(&days_since_epoch)
      .copied()
      .unwrap_or(day.flags);
    let override_duration = overrides_by_day.get(&days_since_epoch).copied();
    let mut work_target = apply_day_configuration(&mut day, flags, user, override_duration);
    if before_start_date {
      day.required_work_hours = Duration::zero();
      work_target = Duration::zero();
    }
    let stored_events = events_by_day.remove(&days_since_epoch).unwrap_or_default();
    let event_ids = stored_events.iter().map(|event| event.id).collect();
    day.events = stored_events.into_iter().map(|event| event.event).collect();
    days.push(BuiltDay {
      day,
      event_ids,
      before_start_date,
      work_target,
      required_work_hours_overridden: override_duration.is_some(),
    });
    if date == end {
      break;
    }
    date = next_date(date)?;
  }

  Ok(days)
}

fn build_monthly_stats(
  days: &[BuiltDay],
  month_start: NaiveDate,
  month_end: NaiveDate,
  today: NaiveDate,
) -> MonthlyStats {
  let mut monthly_stats = empty_monthly_stats();
  let mut total_clocked_work = Duration::zero();
  let mut overtime = Duration::zero();
  let mut undertime = Duration::zero();

  for built_day in days {
    let day = &built_day.day;
    if built_day.before_start_date {
      continue;
    }
    if day.date < month_start || day.date > month_end || day.date > today {
      continue;
    }

    let clocked_work = day.clocked_work_duration();
    total_clocked_work += clocked_work;
    if clocked_work > Duration::zero() || !day.events.is_empty() {
      monthly_stats.worked_days += 1;
    }
    if day.is_remote() {
      monthly_stats.remote_days += 1;
    }
    if day.is_day_off() {
      monthly_stats.day_offs += 1;
    }
    if day.is_vacation() {
      monthly_stats.vacation_days += 1;
    }

    let summary = summarize_day(built_day, today);
    if summary.skipped {
      monthly_stats.skipped_days += 1;
    }
    if day.is_weekend() && summary.full_day_worked {
      monthly_stats.full_weekend_work_days += 1;
    }
    if day.is_vacation() && summary.full_day_worked {
      monthly_stats.full_vacation_work_days += 1;
    }
    if is_calendar_regular_required_day(day) {
      match dashboard_balance(day, built_day.before_start_date, summary.skipped) {
        Balance::Overtime(duration) => overtime += duration,
        Balance::UnderTime(duration) => undertime += duration,
        Balance::Exact => {}
      }
    }
  }

  monthly_stats.total_clocked_work = Some(total_clocked_work.into());
  monthly_stats.overtime = Some(overtime.into());
  monthly_stats.undertime = Some(undertime.into());
  monthly_stats
}

fn empty_monthly_stats() -> MonthlyStats {
  MonthlyStats {
    total_clocked_work: Some(Duration::zero().into()),
    overtime: Some(Duration::zero().into()),
    undertime: Some(Duration::zero().into()),
    worked_days: 0,
    remote_days: 0,
    day_offs: 0,
    vacation_days: 0,
    skipped_days: 0,
    full_weekend_work_days: 0,
    full_vacation_work_days: 0,
  }
}

fn user_stats_from_days(
  days: &[BuiltDay],
  overall_start: NaiveDate,
  today: NaiveDate,
) -> Result<(DaySummary, MonthlyStats, MonthlyStats), Status> {
  let today_summary = days
    .iter()
    .find(|built_day| built_day.day.date == today)
    .map(|built_day| summarize_day(built_day, today))
    .ok_or_else(|| Status::internal("Failed to build today's summary"))?;
  let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
    .ok_or_else(|| Status::internal("Failed to determine month start"))?;
  let month_to_date = build_monthly_stats(days, month_start, today, today);
  let overall = build_monthly_stats(days, overall_start, today, today);
  Ok((today_summary, month_to_date, overall))
}

impl StoreServiceImpl {
  async fn build_day(&self, user_id: Uuid, date: NaiveDate) -> Result<Day, Status> {
    self
      .build_days(user_id, date, date)
      .await?
      .into_iter()
      .next()
      .map(|built_day| built_day.day)
      .ok_or_else(|| Status::internal("Failed to build day"))
  }

  async fn build_days(
    &self,
    user_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
  ) -> Result<Vec<BuiltDay>, Status> {
    if start > end {
      return Err(Status::invalid_argument("Date range start is after end"));
    }

    let user = fetch_core_user(&self.db, user_id).await?;
    let start_days = date_to_epoch_days(start);
    let end_days = date_to_epoch_days(end);

    let flag_rows =
      sqlx::query_as::<_, DayFlagRangeRow>(include_str!("queries/fetch_day_flags_range.sql"))
        .bind(user_id)
        .bind(start_days)
        .bind(end_days)
        .fetch_all(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
    let flags_by_day: BTreeMap<i32, DayFlags> = flag_rows
      .into_iter()
      .map(|row| {
        (
          row.date_days,
          DayFlags::from_bits_truncate(row.flags as u32),
        )
      })
      .collect();

    let override_rows = sqlx::query_as::<_, DayRequiredWorkOverrideRangeRow>(include_str!(
      "queries/fetch_day_required_work_overrides_range.sql"
    ))
    .bind(user_id)
    .bind(start_days)
    .bind(end_days)
    .fetch_all(&self.db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?;
    let overrides_by_day: BTreeMap<i32, Duration> = override_rows
      .into_iter()
      .map(|row| {
        (
          row.date_days,
          Duration::seconds(row.required_work_hours_secs),
        )
      })
      .collect();

    let event_rows =
      sqlx::query_as::<_, EventRangeRow>(include_str!("queries/fetch_events_range.sql"))
        .bind(user_id)
        .bind(start_days)
        .bind(end_days)
        .fetch_all(&self.db)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
    let mut events_by_day: BTreeMap<i32, Vec<StoredEvent>> = BTreeMap::new();
    for row in event_rows {
      events_by_day
        .entry(row.date_days)
        .or_default()
        .push(row_to_event(
          row.id,
          row.event_kind,
          row.hour,
          row.minute,
          row.second,
        )?);
    }

    build_days_from_maps(
      &user,
      start,
      end,
      &flags_by_day,
      &overrides_by_day,
      events_by_day,
    )
  }

  async fn build_dashboard(
    &self,
    user_id: Uuid,
    range_start: NaiveDate,
    range_end: NaiveDate,
    month_start: NaiveDate,
    month_end: NaiveDate,
    today: NaiveDate,
  ) -> Result<DashboardResponse, Status> {
    if range_start > range_end || month_start > month_end {
      return Err(Status::invalid_argument("Date range start is after end"));
    }

    let start = range_start.min(month_start);
    let end = range_end.max(month_end);
    let days = self.build_days(user_id, start, end).await?;
    let summaries = days
      .iter()
      .filter(|built_day| built_day.day.date >= range_start && built_day.day.date <= range_end)
      .map(|built_day| summarize_day(built_day, today))
      .collect();
    let monthly_stats = build_monthly_stats(&days, month_start, month_end, today);

    Ok(DashboardResponse {
      days: summaries,
      month_stats: Some(monthly_stats),
    })
  }

  pub(super) async fn build_user_stats(
    &self,
    user_id: Uuid,
    overall_start: NaiveDate,
    today: NaiveDate,
  ) -> Result<(DaySummary, MonthlyStats, MonthlyStats), Status> {
    let query_start = overall_start.min(today);
    let days = self.build_days(user_id, query_start, today).await?;
    user_stats_from_days(&days, overall_start, today)
  }
}

#[tonic::async_trait]
impl StoreService for StoreServiceImpl {
  async fn get_day(
    self: Arc<Self>,
    request: Request<Date>,
  ) -> Result<Response<taptime_schema::Day>, Status> {
    let user_id = self.authenticated_user(&request).await?;
    let date: chrono::NaiveDate = request.into_inner().into();
    let day = self
      .build_days(user_id, date, date)
      .await?
      .into_iter()
      .next()
      .ok_or_else(|| Status::internal("Failed to build day"))?;
    Ok(Response::new(schema_day_from_built_day(&day)))
  }

  async fn get_work_hours(
    self: Arc<Self>,
    request: Request<Date>,
  ) -> Result<Response<prost_types::Duration>, Status> {
    let user_id = self.authenticated_user(&request).await?;
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
    let user_id = self.authenticated_user(&request).await?;
    let date: chrono::NaiveDate = request.into_inner().into();
    let user = fetch_core_user(&self.db, user_id).await?;
    let today = chrono::Utc::now()
      .with_timezone(&user.time_zone)
      .date_naive();
    let built_day = self
      .build_days(user_id, date, date)
      .await?
      .into_iter()
      .next()
      .ok_or_else(|| Status::internal("Failed to build day"))?;
    let day = &built_day.day;
    let clocked_work = day.clocked_work_duration();
    let skipped = !built_day.before_start_date && is_skipped_day(day, today, clocked_work);
    let balance = dashboard_balance(day, built_day.before_start_date, skipped);
    Ok(Response::new(taptime_schema::Balance::from(&balance)))
  }

  async fn get_dashboard(
    self: Arc<Self>,
    request: Request<DashboardRequest>,
  ) -> Result<Response<DashboardResponse>, Status> {
    let user_id = self.authenticated_user(&request).await?;
    let req = request.into_inner();
    let range_start = date_from_request(req.range_start, "range_start")?;
    let range_end = date_from_request(req.range_end, "range_end")?;
    let month_start = date_from_request(req.month_start, "month_start")?;
    let month_end = date_from_request(req.month_end, "month_end")?;
    let today = date_from_request(req.today, "today")?;
    let dashboard = self
      .build_dashboard(
        user_id,
        range_start,
        range_end,
        month_start,
        month_end,
        today,
      )
      .await?;
    Ok(Response::new(dashboard))
  }

  async fn set_flag(
    self: Arc<Self>,
    request: Request<SetFlagRequest>,
  ) -> Result<Response<()>, Status> {
    let user_id = self.authenticated_user(&request).await?;
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

  async fn set_required_work_hours_override(
    self: Arc<Self>,
    request: Request<SetRequiredWorkHoursOverrideRequest>,
  ) -> Result<Response<()>, Status> {
    let user_id = self.authenticated_user(&request).await?;
    let req = request.into_inner();
    let date: chrono::NaiveDate = req
      .date
      .ok_or_else(|| Status::invalid_argument("Missing date"))?
      .into();
    let days = date_to_epoch_days(date);

    if let Some(required_work_hours) = req.required_work_hours {
      let duration = proto_duration_to_chrono(required_work_hours, "required_work_hours")?;
      sqlx::query(include_str!(
        "queries/upsert_day_required_work_override.sql"
      ))
      .bind(user_id)
      .bind(days)
      .bind(duration.num_seconds())
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    } else {
      sqlx::query(include_str!(
        "queries/delete_day_required_work_override.sql"
      ))
      .bind(user_id)
      .bind(days)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
    }

    Ok(Response::new(()))
  }

  async fn add_check_in(
    self: Arc<Self>,
    request: Request<EventRequest>,
  ) -> Result<Response<()>, Status> {
    let user_id = self.authenticated_user(&request).await?;
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
    let user_id = self.authenticated_user(&request).await?;
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

  async fn delete_event(
    self: Arc<Self>,
    request: Request<DeleteEventRequest>,
  ) -> Result<Response<()>, Status> {
    let user_id = self.authenticated_user(&request).await?;
    let req = request.into_inner();
    let event_id: Uuid = req
      .event_id
      .ok_or_else(|| Status::invalid_argument("Missing event_id"))?
      .into();

    let result = sqlx::query(include_str!("queries/delete_event.sql"))
      .bind(event_id)
      .bind(user_id)
      .execute(&self.db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;

    if result.rows_affected() == 0 {
      return Err(Status::not_found("Event not found"));
    }

    Ok(Response::new(()))
  }
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone;
  use taptime_core::UserSettings;

  use super::*;

  fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
  }

  fn lt(hour: u32, minute: u32) -> LocalTime {
    LocalTime::new(hour, minute, 0).unwrap()
  }

  fn make_user() -> User {
    User {
      id: Uuid::nil(),
      name: "Test User".into(),
      email: "test@example.com".into(),
      organization: None,
      time_zone: chrono_tz::UTC,
      created_at: chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
      last_seen: None,
      rfid_uid: None,
      settings: UserSettings {
        weekends: vec![],
        remote_days: vec![],
        ..Default::default()
      },
    }
  }

  fn duration_seconds(duration: &Option<prost_types::Duration>) -> i64 {
    duration
      .as_ref()
      .map(|duration| duration.seconds)
      .unwrap_or(0)
  }

  fn built_day(day: Day, user: &User) -> BuiltDay {
    BuiltDay {
      day,
      event_ids: vec![],
      before_start_date: false,
      work_target: user.settings.required_work_hours,
      required_work_hours_overridden: false,
    }
  }

  #[test]
  fn build_days_from_maps_includes_empty_days_and_applies_flags_events() {
    let user = make_user();
    let start = date(2024, 1, 1);
    let end = date(2024, 1, 3);
    let mut flags_by_day = BTreeMap::new();
    flags_by_day.insert(
      date_to_epoch_days(date(2024, 1, 2)),
      DayFlags::VACATION | DayFlags::REMOTE,
    );
    let mut events_by_day = BTreeMap::new();
    let check_in_id = Uuid::new_v4();
    let check_out_id = Uuid::new_v4();
    events_by_day.insert(
      date_to_epoch_days(date(2024, 1, 3)),
      vec![
        StoredEvent {
          id: check_in_id,
          event: Event::CheckIn(lt(9, 0)),
        },
        StoredEvent {
          id: check_out_id,
          event: Event::CheckOut(lt(12, 0)),
        },
      ],
    );

    let days = build_days_from_maps(
      &user,
      start,
      end,
      &flags_by_day,
      &BTreeMap::new(),
      events_by_day,
    )
    .unwrap();

    assert_eq!(days.len(), 3);
    assert_eq!(days[0].day.date, date(2024, 1, 1));
    assert!(days[0].day.events.is_empty());
    assert!(days[1].day.is_vacation());
    assert!(days[1].day.is_remote());
    assert_eq!(days[1].day.required_work_hours, Duration::zero());
    assert_eq!(days[2].day.clocked_work_duration(), Duration::hours(3));
    assert_eq!(days[2].event_ids, vec![check_in_id, check_out_id]);

    let schema_day = schema_day_from_built_day(&days[2]);
    assert_eq!(
      schema_day.events[0].id,
      Some(taptime_schema::Uuid::from(check_in_id))
    );
    assert_eq!(
      schema_day.events[1].id,
      Some(taptime_schema::Uuid::from(check_out_id))
    );
  }

  #[test]
  fn build_days_from_maps_applies_required_work_override() {
    let user = make_user();
    let start = date(2024, 1, 1);
    let end = date(2024, 1, 2);
    let flags_by_day = BTreeMap::new();
    let mut overrides_by_day = BTreeMap::new();
    overrides_by_day.insert(date_to_epoch_days(start), Duration::hours(7));

    let days = build_days_from_maps(
      &user,
      start,
      end,
      &flags_by_day,
      &overrides_by_day,
      BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(days[0].day.required_work_hours, Duration::hours(7));
    assert_eq!(days[0].work_target, Duration::hours(7));
    assert!(days[0].required_work_hours_overridden);
    assert_eq!(days[1].day.required_work_hours, Duration::hours(8));
    assert_eq!(days[1].work_target, Duration::hours(8));
    assert!(!days[1].required_work_hours_overridden);
  }

  #[test]
  fn days_before_user_start_date_do_not_count() {
    let mut user = make_user();
    user.settings.start_date = Some(date(2024, 1, 2));

    let days = build_days_from_maps(
      &user,
      date(2024, 1, 1),
      date(2024, 1, 2),
      &BTreeMap::new(),
      &BTreeMap::new(),
      BTreeMap::new(),
    )
    .unwrap();

    assert!(days[0].before_start_date);
    assert_eq!(days[0].day.required_work_hours, Duration::zero());
    assert_eq!(days[0].work_target, Duration::zero());
    assert!(!days[1].before_start_date);
    assert_eq!(days[1].day.required_work_hours, Duration::hours(8));

    let summary = summarize_day(&days[0], date(2024, 1, 3));
    assert!(summary.before_start_date);
    assert!(!summary.skipped);
    let skipped_summary = summarize_day(&days[1], date(2024, 1, 3));
    assert!(skipped_summary.skipped);
    assert_eq!(
      Balance::try_from(skipped_summary.balance.as_ref().unwrap()).unwrap(),
      Balance::UnderTime(Duration::hours(8))
    );

    let stats = build_monthly_stats(&days, date(2024, 1, 1), date(2024, 1, 31), date(2024, 1, 3));
    assert_eq!(stats.skipped_days, 1);
    assert_eq!(duration_seconds(&stats.undertime), 8 * 60 * 60);
  }

  #[test]
  fn monthly_stats_counts_skipped_regular_day_but_not_non_required_days() {
    let user = make_user();
    let mut skipped = user.new_day(date(2024, 1, 1));
    let mut day_off = user.new_day(date(2024, 1, 2));
    let mut vacation = user.new_day(date(2024, 1, 3));
    let mut weekend = user.new_day(date(2024, 1, 4));
    let mut worked = user.new_day(date(2024, 1, 5));

    apply_day_flags(&mut skipped, DayFlags::empty(), &user);
    apply_day_flags(&mut day_off, DayFlags::DAY_OFF, &user);
    apply_day_flags(&mut vacation, DayFlags::VACATION, &user);
    apply_day_flags(&mut weekend, DayFlags::WEEKEND, &user);
    worked.add_event(Event::CheckIn(lt(9, 0))).unwrap();
    worked.add_event(Event::CheckOut(lt(17, 0))).unwrap();

    let days = [
      built_day(skipped, &user),
      built_day(day_off, &user),
      built_day(vacation, &user),
      built_day(weekend, &user),
      built_day(worked, &user),
    ];

    let stats = build_monthly_stats(&days, date(2024, 1, 1), date(2024, 1, 31), date(2024, 1, 5));

    assert_eq!(stats.skipped_days, 1);
    assert_eq!(stats.day_offs, 1);
    assert_eq!(stats.vacation_days, 1);
    assert_eq!(stats.worked_days, 1);
    assert_eq!(duration_seconds(&stats.undertime), 8 * 60 * 60 + 30 * 60);
    assert_eq!(duration_seconds(&stats.overtime), 0);
  }

  #[test]
  fn monthly_stats_counts_full_weekend_and_vacation_work() {
    let user = make_user();
    let mut weekend = user.new_day(date(2024, 1, 6));
    let mut vacation = user.new_day(date(2024, 1, 7));

    apply_day_flags(&mut weekend, DayFlags::WEEKEND, &user);
    weekend.add_event(Event::CheckIn(lt(8, 0))).unwrap();
    weekend.add_event(Event::CheckOut(lt(12, 0))).unwrap();
    weekend.add_event(Event::CheckIn(lt(13, 0))).unwrap();
    weekend.add_event(Event::CheckOut(lt(17, 0))).unwrap();

    apply_day_flags(&mut vacation, DayFlags::VACATION, &user);
    vacation.add_event(Event::CheckIn(lt(8, 0))).unwrap();
    vacation.add_event(Event::CheckOut(lt(12, 0))).unwrap();
    vacation.add_event(Event::CheckIn(lt(13, 30))).unwrap();
    vacation.add_event(Event::CheckOut(lt(17, 0))).unwrap();

    let days = [built_day(weekend, &user), built_day(vacation, &user)];

    let stats = build_monthly_stats(
      &days,
      date(2024, 1, 1),
      date(2024, 1, 31),
      date(2024, 1, 31),
    );

    assert_eq!(stats.worked_days, 2);
    assert_eq!(stats.full_weekend_work_days, 1);
    assert_eq!(stats.full_vacation_work_days, 1);
    assert_eq!(duration_seconds(&stats.undertime), 0);
  }

  #[test]
  fn user_stats_separate_month_to_date_from_overall() {
    let user = make_user();
    let january = user.new_day(date(2024, 1, 31));
    let mut february = user.new_day(date(2024, 2, 1));
    february.add_event(Event::CheckIn(lt(9, 0))).unwrap();
    february.add_event(Event::CheckOut(lt(17, 30))).unwrap();
    let days = [built_day(january, &user), built_day(february, &user)];

    let (_, month, overall) =
      user_stats_from_days(&days, date(2024, 1, 31), date(2024, 2, 1)).unwrap();

    assert_eq!(month.worked_days, 1);
    assert_eq!(month.skipped_days, 0);
    assert_eq!(duration_seconds(&month.undertime), 0);
    assert_eq!(overall.worked_days, 1);
    assert_eq!(overall.skipped_days, 1);
    assert_eq!(duration_seconds(&overall.undertime), 8 * 60 * 60);
  }

  #[test]
  fn user_stats_return_zero_overall_for_future_start_date() {
    let user = make_user();
    let today = date(2024, 2, 1);
    let days = [built_day(user.new_day(today), &user)];

    let (_, _, overall) = user_stats_from_days(&days, date(2024, 2, 2), today).unwrap();

    assert_eq!(overall.worked_days, 0);
    assert_eq!(overall.skipped_days, 0);
    assert_eq!(duration_seconds(&overall.total_clocked_work), 0);
    assert_eq!(duration_seconds(&overall.overtime), 0);
    assert_eq!(duration_seconds(&overall.undertime), 0);
  }

  #[test]
  fn user_stats_respect_work_override_and_nonworking_flags() {
    let user = make_user();
    let mut overridden = user.new_day(date(2024, 2, 1));
    let work_target = apply_day_configuration(
      &mut overridden,
      DayFlags::empty(),
      &user,
      Some(Duration::hours(7)),
    );
    overridden.add_event(Event::CheckIn(lt(9, 0))).unwrap();
    overridden.add_event(Event::CheckOut(lt(16, 30))).unwrap();
    let mut remote = user.new_day(date(2024, 2, 2));
    apply_day_flags(&mut remote, DayFlags::REMOTE, &user);
    let days = [
      BuiltDay {
        day: overridden,
        event_ids: vec![],
        before_start_date: false,
        work_target,
        required_work_hours_overridden: true,
      },
      built_day(remote, &user),
    ];

    let (today, month, _) =
      user_stats_from_days(&days, date(2024, 2, 1), date(2024, 2, 2)).unwrap();

    assert!(today.day.unwrap().flags & DayFlags::REMOTE.bits() != 0);
    assert_eq!(month.remote_days, 1);
    assert_eq!(duration_seconds(&month.overtime), 0);
    assert_eq!(duration_seconds(&month.undertime), 0);
  }

  #[test]
  fn user_stats_return_today_and_zero_activity_counters_for_empty_data() {
    let user = make_user();
    let today = date(2024, 2, 1);
    let days = [built_day(user.new_day(today), &user)];

    let (summary, month, overall) = user_stats_from_days(&days, today, today).unwrap();

    assert!(summary.day.unwrap().events.is_empty());
    assert_eq!(month.worked_days, 0);
    assert_eq!(overall.worked_days, 0);
    assert_eq!(duration_seconds(&month.total_clocked_work), 0);
  }
}
