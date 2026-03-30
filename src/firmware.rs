mod oled;
mod onboard_led;
mod rtc;

use chrono::prelude::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_time::{Duration, Timer};
use embedded_hal::i2c::I2c;

pub use self::{oled::Oled, onboard_led::OnboardLED, rtc::RTC};

pub struct Firmware<I2C> {
  pub spawner: Spawner,
  pub onboard_led: OnboardLED,
  pub rtc: RTC<I2C>,
  pub oled: Oled<I2C>,
}

impl<I2C: I2c> Firmware<I2C> {
  pub async fn init(
    spawner: Spawner,
    onboard_led: OnboardLED,
    rtc: RTC<I2C>,
    oled: Oled<I2C>,
  ) -> Self {
    defmt::info!("Initializing firmware");
    Timer::after(Duration::from_millis(100)).await;
    let firmware = Self {
      spawner,
      onboard_led,
      rtc,
      oled,
    };
    defmt::info!("Firmware initialized");
    firmware
  }

  #[inline]
  pub fn datetime(&self) -> &NaiveDateTime {
    self.rtc.datetime()
  }

  pub async fn run(&mut self) {
    self.rtc.update();

    self.onboard_led.blink(Duration::from_millis(100)).await;

    self.oled.greet_for(Duration::from_millis(3000)).await;
    self
      .oled
      .show_datetime_for(*self.datetime(), Duration::from_millis(3000))
      .await;

    loop {
      let led_fut = self.onboard_led.blink(Duration::from_millis(100));
      let tick_fut = Self::tick(&mut self.rtc, &mut self.oled);
      join(led_fut, tick_fut).await;
    }
  }

  async fn tick(rtc: &mut RTC<I2C>, oled: &mut Oled<I2C>) {
    rtc.update();
    let dt = rtc.datetime();
    oled.set_time(dt.hour() as u8, dt.minute() as u8);
    oled.draw();
    Timer::after(Duration::from_millis(900)).await;
  }

  #[cfg(feature = "clock_set")]
  pub async fn configure_clock(&mut self, datetime_str: &str) {
    self.oled.clear_and_flush();
    let _ = self.onboard_led.blink(Duration::from_millis(100));

    let datetime = DateTime::parse_from_rfc3339(datetime_str)
      .unwrap()
      .naive_local();
    self.rtc.configure_clock(datetime);
    self.oled.show_datetime(*self.datetime());
    Timer::after(Duration::from_millis(5000)).await;
    self.oled.wave_goodbye();
    Timer::after(Duration::from_millis(1000)).await;
    self.oled.clear_and_flush();
  }
}
