bitflags::bitflags! {
  #[repr(transparent)]
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub struct DayFlags: u32 {
    const WEEKEND = 0b00000001;
    const DAY_OFF = 0b00000010;
    const REMOTE  = 0b00000100;
  }
}

impl From<DayFlags> for u32 {
  fn from(flags: DayFlags) -> Self {
    flags.bits()
  }
}

impl From<u32> for DayFlags {
  fn from(bits: u32) -> Self {
    DayFlags::from_bits_truncate(bits)
  }
}
