use embassy_stm32::{
  gpio::{Level, Output, Pin, Speed},
  Peri,
};
use embassy_time::{Duration, Timer};

pub struct OnboardLED {
  led: Output<'static>,
}

impl OnboardLED {
  pub fn new(led: Peri<'static, impl Pin>) -> Self {
    defmt::info!("Initializing onboard LED");
    Self {
      led: Output::new(led, Level::High, Speed::Low),
    }
  }

  #[inline]
  pub fn toggle(&mut self) {
    self.led.toggle();
  }

  #[inline]
  pub fn set_high(&mut self) {
    self.led.set_high();
  }

  #[inline]
  pub fn set_low(&mut self) {
    self.led.set_low();
  }

  #[inline]
  pub async fn blink(&mut self, duration: Duration) {
    self.toggle();
    Timer::after(duration).await;
    self.toggle();
  }
}
