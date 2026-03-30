use chrono::prelude::*;
use embassy_time::{Duration, Timer};
use embedded_graphics::{
  mono_font::{
    iso_8859_1::{FONT_10X20, FONT_5X8},
    MonoTextStyle, MonoTextStyleBuilder,
  },
  pixelcolor::BinaryColor,
  prelude::*,
  text::{Baseline, Text},
};
use embedded_hal::i2c::I2c;
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};

type OledDisplay<I2C> =
  Ssd1306<I2CInterface<I2C>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>;

/// OLED display
///
/// Running on I2C1 (`PB6` SCL, `PB7` SDA)
pub struct Oled<I2C> {
  display: OledDisplay<I2C>,
  text_style: MonoTextStyle<'static, BinaryColor>,
  heading_style: MonoTextStyle<'static, BinaryColor>,
  hh_mm: (u8, u8),
}

impl<I2C: I2c> Oled<I2C> {
  pub fn new(i2c: I2C) -> Self {
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
      .into_buffered_graphics_mode();
    display.init().expect("Cannot init OLED");

    let text_style = MonoTextStyleBuilder::new()
      .font(&FONT_5X8)
      .text_color(BinaryColor::On)
      .build();
    let heading_style = MonoTextStyleBuilder::new()
      .font(&FONT_10X20)
      .text_color(BinaryColor::On)
      .build();

    defmt::info!("Initializing 128x32 OLED");

    Self {
      display,
      text_style,
      heading_style,
      hh_mm: (0, 0),
    }
  }

  #[inline]
  pub fn set_time(&mut self, hours: u8, minutes: u8) {
    self.hh_mm = (hours, minutes);
  }

  #[inline]
  pub fn clear(&mut self) {
    self
      .display
      .clear(BinaryColor::Off)
      .expect("Cannot clear OLED");
  }

  #[inline]
  pub fn flush(&mut self) {
    self.display.flush().expect("Cannot flush OLED");
  }

  #[inline]
  pub fn clear_and_flush(&mut self) {
    self.clear();
    self.flush();
  }

  pub fn show_datetime(&mut self, datetime: NaiveDateTime) {
    self.clear();

    let time = alloc::format!(
      "{:02}:{:02}:{:02}",
      datetime.hour(),
      datetime.minute(),
      datetime.second()
    );
    let date = alloc::format!(
      "{:04}-{:02}-{:02}",
      datetime.year(),
      datetime.month(),
      datetime.day()
    );

    Text::with_baseline(
      date.as_str(),
      Point::new(0, 12),
      self.text_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw date on OLED");

    Text::with_baseline(
      time.as_str(),
      Point::new(0, 32),
      self.heading_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw time on OLED");

    self.flush();
  }

  pub async fn show_datetime_for(&mut self, datetime: NaiveDateTime, duration: Duration) {
    self.show_datetime(datetime);
    Timer::after(duration).await;
    self.clear_and_flush();
  }

  pub fn greet(&mut self) {
    self.clear();

    Text::with_baseline(
      "Greetings!",
      Point::new(13, 28),
      self.heading_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw greeting on OLED");

    self.flush();
  }

  pub async fn greet_for(&mut self, duration: Duration) {
    self.greet();
    Timer::after(duration).await;
    self.clear_and_flush();
  }

  #[allow(unused)]
  pub fn wave_goodbye(&mut self) {
    self.clear();

    Text::with_baseline(
      "Goodbye!",
      Point::new(20, 32),
      self.heading_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw goodbye on OLED");

    self.flush();
  }

  pub fn show_uid(&mut self, uid: &super::Uid) {
    self.clear();

    // Top line: label
    Text::with_baseline(
      "RFID TAG",
      Point::new(24, 8),
      self.text_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw UID label");

    // Bottom line: hex UID e.g. "A1 B2 C3 D4"
    let mut buf = alloc::string::String::new();
    for (i, byte) in uid.as_slice().iter().enumerate() {
      if i > 0 {
        buf.push(' ');
      }
      let _ = core::fmt::write(&mut buf, format_args!("{:02X}", byte));
    }

    Text::with_baseline(
      buf.as_str(),
      Point::new(0, 17),
      self.text_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw UID on OLED");

    self.flush();
  }

  pub fn draw(&mut self) {
    self.clear();

    let time = alloc::format!("{:02}:{:02}", self.hh_mm.0, self.hh_mm.1);
    Text::with_baseline(
      time.as_str(),
      Point::new(40, 32),
      self.heading_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw time on OLED");

    self.flush();
  }
}
