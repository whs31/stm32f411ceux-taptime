use embedded_hal::spi::SpiDevice;
use mfrc522::{
  comm::blocking::spi::{DummyDelay, SpiInterface},
  Initialized, Mfrc522, RxGain,
};

pub struct Uid {
  pub bytes: [u8; 10],
  pub len: usize,
}

impl Uid {
  pub fn as_slice(&self) -> &[u8] {
    &self.bytes[..self.len]
  }
}

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

  pub fn poll(&mut self) -> Option<Uid> {
    // REQA: probe for ISO14443A cards in field
    let atqa = self.mfrc522.reqa().ok()?;

    // SELECT: get UID
    let uid = self.mfrc522.select(&atqa).ok()?;

    let bytes_slice = uid.as_bytes();
    let len = bytes_slice.len().min(10);
    let mut bytes = [0u8; 10];
    bytes[..len].copy_from_slice(&bytes_slice[..len]);

    // HALT: put card back to idle so next poll works
    let _ = self
      .mfrc522
      .hlta()
      .inspect_err(|e| defmt::error!("Error halting RFID"));
    let _ = self
      .mfrc522
      .stop_crypto1()
      .inspect_err(|e| defmt::error!("Error stopping crypto1"));

    Some(Uid { bytes, len })
  }
}
