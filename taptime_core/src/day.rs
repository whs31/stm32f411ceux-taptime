use chrono::NaiveDate;

use crate::{Balance, DayFlags, Error, Event, LocalTime, Result};

#[derive(Debug, Clone)]
pub struct Day {
  pub date: NaiveDate,
  pub events: Vec<Event>,
  pub flags: DayFlags,
  pub required_work_hours: chrono::Duration,
  pub lunch_break_duration: chrono::Duration,
}

impl Day {
  #[inline]
  pub fn is_weekend(&self) -> bool {
    self.flags.contains(DayFlags::WEEKEND)
  }

  #[inline]
  pub fn set_weekend(&mut self) {
    self.flags.insert(DayFlags::WEEKEND);
  }

  #[inline]
  pub fn clear_weekend(&mut self) {
    self.flags.remove(DayFlags::WEEKEND);
  }

  #[inline]
  pub fn is_remote(&self) -> bool {
    self.flags.contains(DayFlags::REMOTE)
  }

  #[inline]
  pub fn set_remote(&mut self) {
    self.flags.insert(DayFlags::REMOTE);
  }

  #[inline]
  pub fn clear_remote(&mut self) {
    self.flags.remove(DayFlags::REMOTE);
  }

  #[inline]
  pub fn is_day_off(&self) -> bool {
    self.flags.contains(DayFlags::DAY_OFF)
  }

  #[inline]
  pub fn set_day_off(&mut self) {
    self.flags.insert(DayFlags::DAY_OFF);
  }

  #[inline]
  pub fn clear_day_off(&mut self) {
    self.flags.remove(DayFlags::DAY_OFF);
  }

  #[inline]
  pub fn first_check_in(&self) -> Option<&LocalTime> {
    self.events.iter().find_map(|e| match e {
      Event::CheckIn(time) => Some(time),
      _ => None,
    })
  }

  #[inline]
  pub fn last_check_out(&self) -> Option<&LocalTime> {
    self.events.iter().rev().find_map(|e| match e {
      Event::CheckOut(time) => Some(time),
      _ => None,
    })
  }

  pub fn add_event(&mut self, event: Event) -> Result<()> {
    match event {
      Event::CheckIn(time) => self.add_check_in(time),
      Event::CheckOut(time) => self.add_check_out(time),
    }
  }

  pub fn add_check_in(&mut self, time: LocalTime) -> Result<()> {
    if self.events.last().map(|e| e.is_check_in()).unwrap_or(false) {
      return Err(Error::InvalidCheckInOutSequence(
        self.events.last().unwrap().clone(),
        Event::CheckIn(time),
      ));
    }
    self.events.push(Event::CheckIn(time));
    Ok(())
  }

  pub fn add_check_out(&mut self, time: LocalTime) -> Result<()> {
    if self
      .events
      .last()
      .map(|e| e.is_check_out())
      .unwrap_or(false)
    {
      return Err(Error::InvalidCheckInOutSequence(
        self.events.last().unwrap().clone(),
        Event::CheckOut(time),
      ));
    }
    self.events.push(Event::CheckOut(time));
    Ok(())
  }

  /// Calculates the total work hours for the day.
  ///
  /// If the day is a day off, returns `None`.
  /// If the day is a weekend, returns the weekend work hours.
  /// If the day is a remote work day, returns the required work hours.
  /// If the day is a regular work day, returns the regular work hours.
  ///
  /// Work hours are calculated by taking the difference between the **first** check-in and
  /// **last** check-out, subtracting the lunch break duration (taken from the user's settings)
  /// if applicable.
  pub fn work_hours(&self) -> Result<Option<chrono::Duration>> {
    if self.is_weekend() {
      return Ok(None);
    }
    if self.is_remote() {
      return Ok(Some(self.required_work_hours));
    }
    let (check_in, check_out) = match (self.first_check_in(), self.last_check_out()) {
      (None, _) => return Ok(None),
      (Some(_), None) => return Err(Error::CheckOutNotFound),
      (Some(in_lt), Some(out_lt)) => (in_lt.to_naive_time()?, out_lt.to_naive_time()?),
    };
    let work_hours = check_out - check_in;
    let lunch_break = if self.lunch_break_duration() > chrono::Duration::zero() {
      self.lunch_break_duration()
    } else {
      chrono::Duration::zero()
    };
    Ok(Some(work_hours - lunch_break))
  }

  #[inline]
  pub fn required_work_hours(&self) -> chrono::Duration {
    self.required_work_hours
  }

  #[inline]
  pub fn lunch_break_duration(&self) -> chrono::Duration {
    self.lunch_break_duration
  }

  #[inline]
  pub fn balance(&self) -> Result<Balance> {
    Ok(Balance::calculate(
      self.work_hours()?.unwrap_or(chrono::Duration::zero()),
      self.required_work_hours(),
    ))
  }
}

impl TryFrom<&taptime_schema::Day> for Day {
  type Error = Error;

  fn try_from(value: &taptime_schema::Day) -> std::result::Result<Self, Self::Error> {
    let date: NaiveDate = value
      .date
      .ok_or(Error::Schema(taptime_schema::Error::MissingField("date")))?
      .into();
    let events = value
      .events
      .iter()
      .map(|e| Event::try_from(e).map_err(Error::Schema))
      .collect::<Result<_>>()?;
    let required_work_hours: chrono::Duration = value
      .required_work_hours
      .map(|d| chrono::Duration::new(d.seconds, d.nanos as u32).unwrap_or_default())
      .ok_or(Error::Schema(taptime_schema::Error::MissingField(
        "required_work_hours",
      )))?;
    let lunch_break_duration: chrono::Duration = value
      .lunch_break_duration
      .map(|d| chrono::Duration::new(d.seconds, d.nanos as u32).unwrap_or_default())
      .ok_or(Error::Schema(taptime_schema::Error::MissingField(
        "lunch_break_duration",
      )))?;
    Ok(Self {
      date,
      events,
      flags: value.flags.into(),
      required_work_hours,
      lunch_break_duration,
    })
  }
}

impl From<&Day> for taptime_schema::Day {
  fn from(day: &Day) -> Self {
    taptime_schema::Day {
      date: Some(day.date.into()),
      events: day.events.iter().map(|e| e.into()).collect(),
      flags: day.flags.into(),
      required_work_hours: Some(day.required_work_hours.into()),
      lunch_break_duration: Some(day.lunch_break_duration.into()),
    }
  }
}

#[cfg(test)]
mod tests {
  use chrono::NaiveDate;
  use uuid::Uuid;

  use super::*;
  use crate::{Balance, Error, Event, User, UserSettings};

  fn make_user() -> User {
    User {
      id: Uuid::new_v4(),
      name: "Test".to_string(),
      email: "test@example.com".to_string(),
      organization: None,
      time_zone: chrono_tz::UTC,
      created_at: chrono::Utc::now(),
      last_seen: None,
      rfid_uid: None,
      settings: UserSettings::default(),
    }
  }

  fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
  }

  fn lt(h: u32, m: u32) -> LocalTime {
    LocalTime::new(h, m, 0).unwrap()
  }

  #[test]
  fn weekend_flag_roundtrip() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    assert!(!day.is_weekend());
    day.set_weekend();
    assert!(day.is_weekend());
    day.clear_weekend();
    assert!(!day.is_weekend());
  }

  #[test]
  fn remote_flag_roundtrip() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    assert!(!day.is_remote());
    day.set_remote();
    assert!(day.is_remote());
    day.clear_remote();
    assert!(!day.is_remote());
  }

  #[test]
  fn day_off_flag_roundtrip() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    assert!(!day.is_day_off());
    day.set_day_off();
    assert!(day.is_day_off());
    day.clear_day_off();
    assert!(!day.is_day_off());
  }

  #[test]
  fn flags_are_independent() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.set_weekend();
    day.set_remote();
    assert!(day.is_weekend());
    assert!(day.is_remote());
    assert!(!day.is_day_off());
    day.clear_weekend();
    assert!(!day.is_weekend());
    assert!(day.is_remote());
  }

  #[test]
  fn first_check_in_on_empty_day_is_none() {
    assert!(
      make_user()
        .new_day(date(2024, 1, 1))
        .first_check_in()
        .is_none()
    );
  }

  #[test]
  fn last_check_out_on_empty_day_is_none() {
    assert!(
      make_user()
        .new_day(date(2024, 1, 1))
        .last_check_out()
        .is_none()
    );
  }

  #[test]
  fn first_check_in_returns_earliest_across_multiple_pairs() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(9, 0)).unwrap();
    day.add_check_out(lt(12, 0)).unwrap();
    day.add_check_in(lt(13, 0)).unwrap();
    day.add_check_out(lt(17, 0)).unwrap();
    assert_eq!(day.first_check_in().unwrap(), &lt(9, 0));
  }

  #[test]
  fn last_check_out_returns_latest_across_multiple_pairs() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(9, 0)).unwrap();
    day.add_check_out(lt(12, 0)).unwrap();
    day.add_check_in(lt(13, 0)).unwrap();
    day.add_check_out(lt(17, 0)).unwrap();
    assert_eq!(day.last_check_out().unwrap(), &lt(17, 0));
  }

  #[test]
  fn consecutive_check_ins_are_rejected() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(9, 0)).unwrap();
    assert!(matches!(
      day.add_check_in(lt(10, 0)),
      Err(Error::InvalidCheckInOutSequence(_, _))
    ));
  }

  #[test]
  fn consecutive_check_outs_are_rejected() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(9, 0)).unwrap();
    day.add_check_out(lt(17, 0)).unwrap();
    assert!(matches!(
      day.add_check_out(lt(18, 0)),
      Err(Error::InvalidCheckInOutSequence(_, _))
    ));
  }

  #[test]
  fn add_event_delegates_to_correct_variant() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_event(Event::CheckIn(lt(9, 0))).unwrap();
    day.add_event(Event::CheckOut(lt(17, 0))).unwrap();
    assert_eq!(day.events.len(), 2);
    assert!(day.events[0].is_check_in());
    assert!(day.events[1].is_check_out());
  }

  // --- work_hours tests ---

  #[test]
  fn work_hours_with_no_events_is_none() {
    assert!(
      make_user()
        .new_day(date(2024, 1, 1))
        .work_hours()
        .unwrap()
        .is_none()
    );
  }

  #[test]
  fn work_hours_on_weekend_is_none() {
    let mut day = make_user().new_day(date(2024, 1, 6));
    day.add_check_in(lt(9, 0)).unwrap();
    day.add_check_out(lt(17, 0)).unwrap();
    day.set_weekend();
    assert!(day.work_hours().unwrap().is_none());
  }

  #[test]
  fn work_hours_on_remote_is_zero_balance() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(9, 0)).unwrap();
    day.add_check_out(lt(17, 0)).unwrap();
    day.set_remote();
    assert_eq!(day.balance().unwrap(), Balance::Exact);
  }

  #[test]
  fn work_hours_check_in_without_check_out_is_error() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(9, 0)).unwrap();
    assert!(matches!(day.work_hours(), Err(Error::CheckOutNotFound)));
  }

  #[test]
  fn work_hours_subtracts_lunch_break() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(9, 0)).unwrap();
    day.add_check_out(lt(17, 0)).unwrap(); // 8h raw - 30min = 7h30min
    assert_eq!(
      day.work_hours().unwrap().unwrap(),
      chrono::Duration::minutes(450)
    );
  }

  #[test]
  fn work_hours_uses_first_check_in_and_last_check_out() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(8, 0)).unwrap();
    day.add_check_out(lt(12, 0)).unwrap();
    day.add_check_in(lt(13, 0)).unwrap();
    day.add_check_out(lt(18, 0)).unwrap(); // span 8:00-18:00 = 10h - 30min = 9h30min
    assert_eq!(
      day.work_hours().unwrap().unwrap(),
      chrono::Duration::minutes(570)
    );
  }

  #[test]
  fn balance_overtime() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(8, 0)).unwrap();
    day.add_check_out(lt(17, 0)).unwrap(); // 9h - 30min = 8h30min => +30min
    assert_eq!(
      day.balance().unwrap(),
      Balance::Overtime(chrono::Duration::minutes(30))
    );
  }

  #[test]
  fn balance_undertime() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(9, 0)).unwrap();
    day.add_check_out(lt(16, 0)).unwrap(); // 7h - 30min = 6h30min => -1h30min
    assert_eq!(
      day.balance().unwrap(),
      Balance::UnderTime(chrono::Duration::minutes(90))
    );
  }

  #[test]
  fn balance_exact() {
    let mut day = make_user().new_day(date(2024, 1, 1));
    day.add_check_in(lt(8, 0)).unwrap();
    day.add_check_out(lt(16, 30)).unwrap(); // 8h30min - 30min = 8h exact
    assert_eq!(day.balance().unwrap(), Balance::Exact);
  }

  #[test]
  fn balance_weekend_with_no_events_is_full_undertime() {
    let mut day = make_user().new_day(date(2024, 1, 6));
    day.set_weekend(); // work_hours => None => 0h worked
    assert_eq!(
      day.balance().unwrap(),
      Balance::UnderTime(chrono::Duration::hours(8))
    );
  }
}
