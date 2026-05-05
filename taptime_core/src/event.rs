use crate::LocalTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
  CheckIn(LocalTime),
  CheckOut(LocalTime),
}

impl Event {
  #[inline]
  pub fn time(&self) -> &LocalTime {
    match self {
      Event::CheckIn(time) => time,
      Event::CheckOut(time) => time,
    }
  }

  #[inline]
  pub fn is_check_in(&self) -> bool {
    matches!(self, Event::CheckIn(_))
  }

  #[inline]
  pub fn is_check_out(&self) -> bool {
    matches!(self, Event::CheckOut(_))
  }
}
