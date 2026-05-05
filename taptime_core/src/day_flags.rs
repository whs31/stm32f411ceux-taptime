bitflags::bitflags! {
  #[repr(transparent)]
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub struct DayFlags: u32 {
    const WEEKEND = 0b00000001;
    const DAY_OFF = 0b00000010;
    const REMOTE  = 0b00000100;
  }
}
