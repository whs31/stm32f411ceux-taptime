mod buzzer;
mod oled;
mod onboard_led;
mod rfid;
mod rtc;
mod wifi;

use chrono::prelude::*;
use embassy_executor::Spawner;
use embassy_futures::{
  join::join,
  select::{select, Either},
};
use embassy_stm32::time::Hertz;
use embassy_time::{Duration, Ticker, Timer};
use embedded_hal::{i2c::I2c, spi::SpiDevice};

pub use self::{
  buzzer::Buzzer,
  oled::{Oled, WifiStatus},
  onboard_led::OnboardLED,
  rfid::{Uid, RFID},
  rtc::RTC,
  wifi::Wifi,
};

const WIFI_SSID: &str = "McDonald's Wi-Fi Free";
const WIFI_PASSWORD: &str = "013214415";

pub struct Firmware<I2C, SPI: SpiDevice> {
  pub _spawner: Spawner,
  pub onboard_led: OnboardLED,
  pub rtc: RTC<I2C>,
  pub oled: Oled<I2C>,
  pub buzzer: Buzzer<'static, embassy_stm32::peripherals::TIM4>,
  pub rfid: RFID<SPI>,
  pub wifi: Wifi,
}

impl<I2C: I2c, SPI: SpiDevice> Firmware<I2C, SPI> {
  pub async fn init(
    spawner: Spawner,
    onboard_led: OnboardLED,
    rtc: RTC<I2C>,
    oled: Oled<I2C>,
    buzzer: Buzzer<'static, embassy_stm32::peripherals::TIM4>,
    rfid: RFID<SPI>,
    wifi: Wifi,
  ) -> Self {
    defmt::info!("Initializing firmware");
    Timer::after(Duration::from_millis(100)).await;
    let firmware = Self {
      _spawner: spawner,
      onboard_led,
      rtc,
      oled,
      buzzer,
      rfid,
      wifi,
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
    self.buzzer.boot_chime().await;

    self.oled.greet_for(Duration::from_millis(1000)).await;
    self
      .oled
      .show_datetime_for(*self.datetime(), Duration::from_millis(2000))
      .await;

    self.oled.set_wifi_status(WifiStatus::Connecting);
    let connected = {
      let mut connect_fut = core::pin::pin!(self.wifi.connect(WIFI_SSID, WIFI_PASSWORD));
      let mut ticker = Ticker::every(Duration::from_millis(100));
      let mut frame = 0u8;
      loop {
        match select(connect_fut.as_mut(), ticker.next()).await {
          Either::First(result) => break result,
          Either::Second(_) => {
            self.oled.draw_wifi_connecting(frame);
            frame = frame.wrapping_add(1);
          }
        }
      }
    };

    if connected {
      defmt::info!("WiFi connected");
      self.oled.set_wifi_status(WifiStatus::Connected);
      self.oled.show_status_detail("WiFi", "OK", true);
      Timer::after(Duration::from_millis(1000)).await;
    } else {
      defmt::warn!("WiFi failed");
      self.oled.set_wifi_status(WifiStatus::Failed);
      self.oled.show_status_detail("WiFi", "FAIL", false);
      Timer::after(Duration::from_millis(2000)).await;
    }
    self.oled.clear_and_flush();

    loop {
      let led_fut = self.onboard_led.blink(Duration::from_millis(100));
      let tick_fut = Self::tick(
        &mut self.rtc,
        &mut self.oled,
        &mut self.rfid,
        &mut self.buzzer,
      );
      join(led_fut, tick_fut).await;
    }
  }

  async fn tick(
    rtc: &mut RTC<I2C>,
    oled: &mut Oled<I2C>,
    rfid: &mut RFID<SPI>,
    buzzer: &mut Buzzer<'static, embassy_stm32::peripherals::TIM4>,
  ) {
    // Poll RFID rapidly for up to 900ms before doing a clock update
    let deadline = embassy_time::Instant::now() + Duration::from_millis(900);
    loop {
      if let Some(uid) = rfid.poll() {
        defmt::info!("RFID tap: {:02X}", uid.as_slice());
        buzzer.beep(Hertz(1760), Duration::from_millis(80)).await;
        oled.show_uid(&uid);
        Timer::after(Duration::from_millis(3000)).await;
        break;
      }
      Timer::after(Duration::from_millis(50)).await;
      if embassy_time::Instant::now() >= deadline {
        break;
      }
    }

    rtc.update();
    let dt = rtc.datetime();
    oled.set_time(dt.hour() as u8, dt.minute() as u8);
    oled.draw();
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
