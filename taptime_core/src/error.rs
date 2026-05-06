use crate::Event;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("invalid time: {0}:{1}:{2}")]
  InvalidTime(u32, u32, u32),

  #[error("check-in must be followed by a check-out and vise versa, got {0:?} followed by {1:?}")]
  InvalidCheckInOutSequence(Event, Event),

  #[error("uid has uneven length ({0})")]
  UidUnevenLength(usize),

  #[error("invalid uid hex string")]
  InvalidUidHex,

  #[error("invalid uid length (expected 4, 7, or 10 bytes, got {0})")]
  InvalidUidLength(usize),

  #[error("found check-in without subsequent check-out")]
  CheckOutNotFound,

  #[error(transparent)]
  Schema(#[from] taptime_schema::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
