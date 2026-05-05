use chrono::NaiveDate;

use crate::{Balance, DayFlags, Error, Event, LocalTime, Result, User};

#[derive(Debug, Clone)]
pub struct Day {
  pub date: NaiveDate,
  pub events: Vec<Event>,
  pub flags: DayFlags,
  pub required_work_hours: chrono::Duration,
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
  /// If the day is a day off or remote work day, always returns `None`.
  /// If the day is a weekend, returns the weekend work hours.
  /// If the day is a regular work day, returns the regular work hours.
  ///
  /// Work hours are calculated by taking the difference between the **first** check-in and
  /// **last** check-out, subtracting the lunch break duration (taken from the user's settings)
  /// if applicable.
  pub fn work_hours(&self, user: &User) -> Result<Option<chrono::Duration>> {
    if self.is_remote() || self.is_weekend() {
      return Ok(None);
    }
    let (check_in, check_out) = match (self.first_check_in(), self.last_check_out()) {
      (None, _) => return Ok(None),
      (Some(_), None) => return Err(Error::CheckOutNotFound),
      (Some(in_lt), Some(out_lt)) => (in_lt.to_naive_time()?, out_lt.to_naive_time()?),
    };
    let work_hours = check_out - check_in;
    let lunch_break = if user.settings.lunch_break_duration > chrono::Duration::zero() {
      user.settings.lunch_break_duration
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
  pub fn balance(&self, user: &User) -> Result<Balance> {
    Ok(Balance::calculate(
      self.work_hours(user)?.unwrap_or(chrono::Duration::zero()),
      user.settings.required_work_hours,
    ))
  }
}
