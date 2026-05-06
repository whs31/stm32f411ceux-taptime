#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("weekday must be specified")]
  InvalidWeekday,

  #[error("invalid or unknown timezone: {0}")]
  InvalidTz(String),

  #[error("required field missing: {0}")]
  MissingField(&'static str),

  #[error("generic proto schema error: {0}")]
  Generic(String),
}

pub type Result<T> = std::result::Result<T, Error>;
