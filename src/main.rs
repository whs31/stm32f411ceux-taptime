#![no_std]
#![no_main]

use core::cell::RefCell;
use chrono::prelude::*;
mod heap;
mod modules;

extern crate alloc;

#[allow(unused_imports)]
use defmt_rtt as _;
use ds323x::{DateTimeAccess, Ds323x};
use embassy_executor::Spawner;
use embassy_stm32::{
  bind_interrupts, dma,
  gpio::{Level, Output, Speed},
  i2c::{self, I2c},
  peripherals,
  spi::{Config as SpiConfig, Spi},
  time::Hertz,
};
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::i2c::RefCellDevice;
use embedded_hal_bus::spi::ExclusiveDevice;
#[allow(unused_imports)]
use panic_probe as _;

bind_interrupts!(struct Irqs {
  I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
  I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
  I2C2_EV => i2c::EventInterruptHandler<peripherals::I2C2>;
  I2C2_ER => i2c::ErrorInterruptHandler<peripherals::I2C2>;
  DMA1_STREAM6 => dma::InterruptHandler<peripherals::DMA1_CH6>;
  DMA1_STREAM0 => dma::InterruptHandler<peripherals::DMA1_CH0>;
  DMA1_STREAM7 => dma::InterruptHandler<peripherals::DMA1_CH7>;
  DMA1_STREAM2 => dma::InterruptHandler<peripherals::DMA1_CH2>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
  heap::init();

  let p = embassy_stm32::init(Default::default());
  Timer::after(Duration::from_millis(100)).await;

  defmt::info!("Starting");

  let mut i2c1_config = i2c::Config::default();
  i2c1_config.frequency = Hertz(400_000);

  // I2C1: PB6=SCL, PB7=SDA @ 400 kHz — OLED
  let i2c1 = I2c::new(
    p.I2C1,
    p.PB6,
    p.PB7,
    p.DMA1_CH6,
    p.DMA1_CH0,
    Irqs,
    i2c1_config,
  );


  let i2c1_bus = RefCell::new(i2c1);
  let rtc_dev = RefCellDevice::new(&i2c1_bus);
  let oled_dev = RefCellDevice::new(&i2c1_bus);

  let mut rtc = Ds323x::new_ds3231(rtc_dev);
  defmt::info!("RTC initialized");

  // SPI1: PA5=SCK, PA7=MOSI, PA6=MISO @ 1 MHz — MFRC522
  let mut spi_config = SpiConfig::default();
  spi_config.frequency = Hertz(1_000_000);
  let spi = Spi::new_blocking(p.SPI1, p.PA5, p.PA7, p.PA6, spi_config);
  let cs = Output::new(p.PB12, Level::High, Speed::VeryHigh);
  let _spi_dev = ExclusiveDevice::new(spi, cs, Delay);
  // let itf = SpiInterface::new(_spi_dev);
  // let mut mfrc522 = Mfrc522::new(itf).init().expect("could not create MFRC522");

  let mut oled = modules::Oled::new(oled_dev);
  oled.clear();
  defmt::info!("OLED initialized");

  let mut led = Output::new(p.PC13, Level::High, Speed::Low);

  #[cfg(feature = "clock_set")]
  {
    rtc
      .set_datetime(
        &DateTime::parse_from_rfc3339(build_time::build_time_local!())
          .unwrap()
          .naive_local(),
      )
      .expect("Could not set RTC datetime");
    defmt::info!(
      "RTC datetime set to {:?}",
      rtc.datetime().expect("Could not get RTC datetime")
    );
    oled.show_datetime(rtc.datetime().expect("Could not get RTC datetime"));
    led.set_high();
    Timer::after(Duration::from_millis(5000)).await;
    led.set_low();
    oled.wave_goodbye();
    Timer::after(Duration::from_millis(1000)).await;
    oled.clear();
    oled.flush();
    return;
  }

  loop {
    led.toggle();
    Timer::after(Duration::from_millis(1000)).await;

    let datetime = rtc.datetime().expect("Could not get RTC datetime");
    oled.set_time(datetime.hour() as u8, datetime.minute() as u8);
    oled.draw();
  }
}
