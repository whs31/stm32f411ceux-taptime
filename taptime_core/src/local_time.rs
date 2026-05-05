use chrono::{NaiveTime, Timelike};

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalTime {
  pub hour: u32,
  pub minute: u32,
  pub second: u32,
}

impl LocalTime {
  pub fn new(hour: u32, minute: u32, second: u32) -> Result<Self> {
    if hour > 23 || minute > 59 || second > 59 {
      return Err(Error::InvalidTime(hour, minute, second));
    }
    Ok(Self {
      hour,
      minute,
      second,
    })
  }

  pub fn to_naive_time(&self) -> Result<NaiveTime> {
    NaiveTime::from_hms_opt(self.hour, self.minute, self.second).ok_or(Error::InvalidTime(
      self.hour,
      self.minute,
      self.second,
    ))
  }

  pub fn from_naive_time(time: NaiveTime) -> Self {
    Self {
      hour: time.hour(),
      minute: time.minute(),
      second: time.second(),
    }
  }
}
