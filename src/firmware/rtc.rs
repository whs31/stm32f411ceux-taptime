use chrono::NaiveDateTime;
use ds323x::{ic, interface::I2cInterface, DateTimeAccess, Ds323x};
use embedded_hal::i2c::I2c;

pub struct RTC<I2C> {
  rtc: Ds323x<I2cInterface<I2C>, ic::DS3231>,
  datetime: NaiveDateTime,
}

impl<I2C: I2c> RTC<I2C> {
  pub fn new(rtc: I2C) -> Self {
    Self {
      rtc: Ds323x::new_ds3231(rtc),
      datetime: NaiveDateTime::default(),
    }
  }

  #[inline]
  pub fn datetime(&self) -> &NaiveDateTime {
    &self.datetime
  }

  pub fn update(&mut self) -> &NaiveDateTime {
    self.datetime = self.rtc.datetime().unwrap_or_default();
    &self.datetime
  }

  pub fn configure_clock(&mut self, datetime: NaiveDateTime) {
    self.rtc.set_datetime(&datetime).unwrap();
    self.datetime = datetime;
  }
}
