use embedded_hal::spi::SpiDevice;
use mfrc522::{
  comm::blocking::spi::{DummyDelay, SpiInterface},
  Initialized, Mfrc522,
};

pub struct RFID<SPI>
where
  SPI: SpiDevice,
{
  mfrc522: Mfrc522<SpiInterface<SPI, DummyDelay>, Initialized>,
}

impl<SPI: SpiDevice> RFID<SPI> {
  pub fn new(itf: SpiInterface<SPI, DummyDelay>) -> Self {
    defmt::info!("Initializing RFID");
    let mut rfid = Self {
      mfrc522: Mfrc522::new(itf).init().expect("could not create MFRC522"),
    };
    defmt::info!("RFID hardware version: {}", rfid.hardware_version());
    rfid
  }

  #[inline]
  pub fn hardware_version(&mut self) -> u8 {
    self.mfrc522.version().unwrap_or(0)
  }
}
