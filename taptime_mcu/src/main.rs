#![no_std]
#![no_main]

use core::cell::RefCell;

use embassy_time::Delay;
mod firmware;
mod heap;

extern crate alloc;

#[allow(unused_imports)]
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
  bind_interrupts, dma,
  gpio::{Level, Output, OutputType, Speed},
  i2c::{self, I2c},
  peripherals,
  spi::{Config as SpiConfig, Spi},
  time::Hertz,
  timer::{
    simple_pwm::{PwmPin, SimplePwm},
    Channel,
  },
  usart::{self, Uart},
};
use embedded_hal_bus::{i2c::RefCellDevice, spi::ExclusiveDevice};
use mfrc522::comm::blocking::spi::SpiInterface;
#[allow(unused_imports)]
use panic_probe as _;

bind_interrupts!(struct Irqs {
  I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
  I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
  DMA1_STREAM6 => dma::InterruptHandler<peripherals::DMA1_CH6>;
  DMA1_STREAM0 => dma::InterruptHandler<peripherals::DMA1_CH0>;
  USART1 => usart::InterruptHandler<peripherals::USART1>;
  DMA2_STREAM7 => dma::InterruptHandler<peripherals::DMA2_CH7>;  // USART1 TX
  DMA2_STREAM2 => dma::InterruptHandler<peripherals::DMA2_CH2>;  // USART1 RX
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
  heap::init();

  let p = embassy_stm32::init(Default::default());

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

  // SPI1: PA5=SCK, PA7=MOSI, PA6=MISO @ 1 MHz — MFRC522
  let mut spi_config = SpiConfig::default();
  spi_config.frequency = Hertz(5_000_000);
  let spi = Spi::new_blocking(p.SPI1, p.PA5, p.PA7, p.PA6, spi_config);
  let cs = Output::new(p.PB12, Level::High, Speed::VeryHigh);
  let spi_dev = ExclusiveDevice::new(spi, cs, Delay).unwrap();
  let itf = SpiInterface::new(spi_dev);

  // USART1: PA10=RX, PA9=TX @ 115200 baud — Wi-Fi
  let mut usart_config = usart::Config::default();
  usart_config.baudrate = 115200;
  let uart = Uart::new(
    p.USART1,
    p.PA10,     // RX
    p.PA9,      // TX
    p.DMA2_CH7, // TX DMA
    p.DMA2_CH2, // RX DMA
    Irqs,
    usart_config,
  )
  .unwrap();

  let pwm_pin = PwmPin::new(p.PB8, OutputType::PushPull);
  let pwm = SimplePwm::new(
    p.TIM4,
    None,
    None,
    Some(pwm_pin),
    None,
    Hertz(1000),
    Default::default(),
  );

  let (tx, rx) = uart.split();

  let mut firmware = firmware::Firmware::init(
    spawner,
    firmware::OnboardLED::new(p.PC13),
    firmware::RTC::new(rtc_dev),
    firmware::Oled::new(oled_dev),
    firmware::Buzzer::new(pwm, Channel::Ch3),
    firmware::RFID::new(itf),
    firmware::Wifi::new(tx, rx),
  )
  .await;

  #[cfg(feature = "clock_set")]
  {
    firmware
      .configure_clock(build_time::build_time_local!())
      .await;
    return;
  }

  firmware.run().await;
}
