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

impl TryFrom<&taptime_schema::Event> for Event {
  type Error = taptime_schema::Error;
  fn try_from(value: &taptime_schema::Event) -> Result<Self, taptime_schema::Error> {
    match value.event_type {
      None => Err(taptime_schema::Error::MissingField("event_type")),
      Some(event) => match event {
        taptime_schema::event::EventType::CheckIn(time) => {
          Ok(Self::CheckIn(time.try_into().map_err(
            |e: crate::Error| taptime_schema::Error::Generic(e.to_string()),
          )?))
        }
        taptime_schema::event::EventType::CheckOut(time) => {
          Ok(Self::CheckOut(time.try_into().map_err(
            |e: crate::Error| taptime_schema::Error::Generic(e.to_string()),
          )?))
        }
      },
    }
  }
}

impl TryFrom<taptime_schema::Event> for Event {
  type Error = taptime_schema::Error;
  fn try_from(value: taptime_schema::Event) -> Result<Self, taptime_schema::Error> {
    Self::try_from(&value)
  }
}

impl From<&Event> for taptime_schema::Event {
  fn from(value: &Event) -> Self {
    match value {
      Event::CheckIn(time) => Self {
        event_type: Some(taptime_schema::event::EventType::CheckIn(
          time.clone().into(),
        )),
      },
      Event::CheckOut(time) => Self {
        event_type: Some(taptime_schema::event::EventType::CheckOut(
          time.clone().into(),
        )),
      },
    }
  }
}

impl From<Event> for taptime_schema::Event {
  fn from(value: Event) -> Self {
    Self::from(&value)
  }
}
