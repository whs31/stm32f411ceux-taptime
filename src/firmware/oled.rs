use chrono::prelude::*;
use embassy_time::{Duration, Timer};
use embedded_graphics::{
  image::Image,
  mono_font::{
    iso_8859_1::{FONT_10X20, FONT_5X8},
    MonoTextStyle, MonoTextStyleBuilder,
  },
  pixelcolor::BinaryColor,
  prelude::*,
  text::{Baseline, Text},
};
use embedded_hal::i2c::I2c;
use embedded_iconoir::prelude::IconoirNewIcon;
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};

type OledDisplay<I2C> =
  Ssd1306<I2CInterface<I2C>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>;

#[derive(Clone, Copy, PartialEq)]
pub enum WifiStatus {
  Unknown,
  Connecting,
  Connected,
  Failed,
}

/// OLED display
///
/// Running on I2C1 (`PB6` SCL, `PB7` SDA)
pub struct Oled<I2C> {
  display: OledDisplay<I2C>,
  text_style: MonoTextStyle<'static, BinaryColor>,
  heading_style: MonoTextStyle<'static, BinaryColor>,
  hh_mm: (u8, u8),
  wifi_status: WifiStatus,
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
      wifi_status: WifiStatus::Unknown,
    }
  }

  #[inline]
  pub fn set_time(&mut self, hours: u8, minutes: u8) {
    self.hh_mm = (hours, minutes);
  }

  #[inline]
  pub fn set_wifi_status(&mut self, status: WifiStatus) {
    self.wifi_status = status;
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
    use embedded_iconoir::size32px::animals::Jellyfish;

    self.clear();

    Image::new(&Jellyfish::new(BinaryColor::On), Point::new(0, 0))
      .draw(&mut self.display)
      .expect("Cannot draw Jellyfish on OLED");
    Text::with_baseline(
      "Greetings!",
      Point::new(32, 28),
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

  /// Check-in response: "Hello," / <name>
  pub fn show_tap_checkin(&mut self, name: &str) {
    self.clear();
    Text::with_baseline("Hello,", Point::new(0, 10), self.text_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw check-in label");
    Text::with_baseline(name, Point::new(0, 32), self.heading_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw check-in name");
    self.flush();
  }

  /// Check-out response: <name> / <duration>
  pub fn show_tap_checkout(&mut self, name: &str, duration: &str) {
    self.clear();
    Text::with_baseline(name, Point::new(0, 10), self.text_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw check-out name");
    Text::with_baseline(duration, Point::new(0, 32), self.heading_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw check-out duration");
    self.flush();
  }

  /// Unknown UID response: hex UID / "Unknown!"
  pub fn show_tap_unknown(&mut self, uid_hex: &str) {
    self.clear();
    Text::with_baseline(uid_hex, Point::new(0, 10), self.text_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw unknown UID");
    Text::with_baseline("Unknown!", Point::new(0, 32), self.heading_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw unknown label");
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

  pub fn show_status(&mut self, msg: &str) {
    self.clear();
    Text::with_baseline(msg, Point::new(0, 32), self.heading_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw status on OLED");
    self.flush();
  }

  /// Two-line status: small label on top, value on bottom
  pub fn show_status_detail(&mut self, label: &str, value: &str, ok: bool) {
    use embedded_iconoir::size12px::actions::{CheckCircle, Prohibition};

    self.clear();
    let icon = if ok {
      Image::new(&CheckCircle::new(BinaryColor::On), Point::new(116, 0))
        .draw(&mut self.display)
        .expect("Cannot draw status icon")
    } else {
      Image::new(&Prohibition::new(BinaryColor::On), Point::new(116, 0))
        .draw(&mut self.display)
        .expect("Cannot draw status icon")
    };
    Text::with_baseline(label, Point::new(0, 10), self.text_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw status label");
    Text::with_baseline(
      value,
      Point::new(0, 32),
      self.heading_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw status value");
    self.flush();
  }

  pub fn draw(&mut self) {
    use embedded_iconoir::size12px::connectivity::{PrivateWifi, Wifi, WifiOff, WifiSignalNone};

    self.clear();

    // Clock: "HH:MM" centered (50px wide), bottom row
    let time = alloc::format!("{:02}:{:02}", self.hh_mm.0, self.hh_mm.1);
    Text::with_baseline(
      time.as_str(),
      Point::new(39, 32),
      self.heading_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw time on OLED");

    match self.wifi_status {
      WifiStatus::Unknown => Image::new(&WifiSignalNone::new(BinaryColor::On), Point::new(112, 0))
        .draw(&mut self.display)
        .expect("Cannot draw WiFi indicator"),
      WifiStatus::Connecting => Image::new(&WifiOff::new(BinaryColor::On), Point::new(112, 0))
        .draw(&mut self.display)
        .expect("Cannot draw WiFi indicator"),
      WifiStatus::Connected => Image::new(&Wifi::new(BinaryColor::On), Point::new(112, 0))
        .draw(&mut self.display)
        .expect("Cannot draw WiFi indicator"),
      WifiStatus::Failed => Image::new(&PrivateWifi::new(BinaryColor::On), Point::new(112, 0))
        .draw(&mut self.display)
        .expect("Cannot draw WiFi indicator"),
    };

    self.flush();
  }

  /// Full-screen animated spinner shown while connecting to WiFi.
  /// Call repeatedly with an incrementing `frame` counter (~4 fps looks good).
  pub fn draw_wifi_connecting(&mut self, frame: u8) {
    const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];

    let spinner = FRAMES[(frame as usize) % 4];
    self.clear();
    Text::with_baseline("WiFi", Point::new(0, 10), self.text_style, Baseline::Bottom)
      .draw(&mut self.display)
      .expect("Cannot draw WiFi label");
    Text::with_baseline(
      spinner,
      Point::new(59, 32),
      self.heading_style,
      Baseline::Bottom,
    )
    .draw(&mut self.display)
    .expect("Cannot draw WiFi spinner");
    self.flush();
  }
}
