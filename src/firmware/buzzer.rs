use cortex_m::prelude::*;
use embassy_stm32::{
  time::Hertz,
  timer::{simple_pwm::SimplePwm, Channel},
};
use embassy_time::{Duration, Timer};

pub struct Buzzer<'d, T: embassy_stm32::timer::GeneralInstance4Channel> {
  pwm: SimplePwm<'d, T>,
  channel: Channel,
}

impl<'d, T: embassy_stm32::timer::GeneralInstance4Channel> Buzzer<'d, T> {
  pub fn new(pwm: SimplePwm<'d, T>, channel: Channel) -> Self {
    defmt::info!("Initializing buzzer");
    Self { pwm, channel }
  }

  /// Set frequency and enable output
  pub fn tone(&mut self, freq: Hertz) {
    self.pwm.set_frequency(freq);
    self.pwm.set_duty(self.channel, 50);
    self.pwm.enable(self.channel);
  }

  /// Stop output
  pub fn off(&mut self) {
    self.pwm.disable(self.channel);
  }

  /// Single beep at given frequency for given duration
  pub async fn beep(&mut self, freq: Hertz, duration: Duration) {
    self.tone(freq);
    Timer::after(duration).await;
    self.off();
  }

  /// N beeps with a gap between each
  pub async fn beep_n(&mut self, freq: Hertz, on: Duration, off: Duration, n: u8) {
    for i in 0..n {
      self.beep(freq, on).await;
      if i < n - 1 {
        Timer::after(off).await;
      }
    }
  }

  /// Boot chime: 2 ascending beeps
  pub async fn boot_chime(&mut self) {
    self.beep(Hertz(880), Duration::from_millis(80)).await;
    Timer::after(Duration::from_millis(60)).await;
    self.beep(Hertz(1320), Duration::from_millis(120)).await;
  }
}
