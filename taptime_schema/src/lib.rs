include!(concat!(env!("OUT_DIR"), "/_includes.rs"));

use std::str::FromStr;

pub use com::whs31::taptime::*;

mod error;

pub use self::error::{Error, Result};

impl From<&uuid::Uuid> for Uuid {
  fn from(value: &uuid::Uuid) -> Self {
    let bytes = value.as_bytes();
    Self {
      most_significant_bits: u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
      least_significant_bits: u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
    }
  }
}

impl From<uuid::Uuid> for Uuid {
  fn from(value: uuid::Uuid) -> Self {
    Self::from(&value)
  }
}

impl From<&Uuid> for uuid::Uuid {
  fn from(value: &Uuid) -> Self {
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&value.most_significant_bits.to_be_bytes());
    bytes[8..16].copy_from_slice(&value.least_significant_bits.to_be_bytes());
    uuid::Uuid::from_bytes(bytes)
  }
}

impl From<Uuid> for uuid::Uuid {
  fn from(value: Uuid) -> Self {
    Self::from(&value)
  }
}

impl From<&chrono::NaiveDate> for Date {
  fn from(value: &chrono::NaiveDate) -> Self {
    Self {
      days_since_epoch: value.to_epoch_days(),
    }
  }
}

impl From<chrono::NaiveDate> for Date {
  fn from(value: chrono::NaiveDate) -> Self {
    Self::from(&value)
  }
}

impl From<&Date> for chrono::NaiveDate {
  fn from(value: &Date) -> Self {
    chrono::NaiveDate::from_epoch_days(value.days_since_epoch).unwrap_or_default()
  }
}

impl From<Date> for chrono::NaiveDate {
  fn from(value: Date) -> Self {
    Self::from(&value)
  }
}

impl From<chrono::Weekday> for Weekday {
  fn from(value: chrono::Weekday) -> Self {
    match value {
      chrono::Weekday::Mon => Self::Monday,
      chrono::Weekday::Tue => Self::Tuesday,
      chrono::Weekday::Wed => Self::Wednesday,
      chrono::Weekday::Thu => Self::Thursday,
      chrono::Weekday::Fri => Self::Friday,
      chrono::Weekday::Sat => Self::Saturday,
      chrono::Weekday::Sun => Self::Sunday,
    }
  }
}

impl TryFrom<Weekday> for chrono::Weekday {
  type Error = Error;

  fn try_from(value: Weekday) -> std::result::Result<Self, Self::Error> {
    match value {
      Weekday::Unspecified => Err(Error::InvalidWeekday),
      Weekday::Monday => Ok(chrono::Weekday::Mon),
      Weekday::Tuesday => Ok(chrono::Weekday::Tue),
      Weekday::Wednesday => Ok(chrono::Weekday::Wed),
      Weekday::Thursday => Ok(chrono::Weekday::Thu),
      Weekday::Friday => Ok(chrono::Weekday::Fri),
      Weekday::Saturday => Ok(chrono::Weekday::Sat),
      Weekday::Sunday => Ok(chrono::Weekday::Sun),
    }
  }
}

impl From<&chrono_tz::Tz> for Tz {
  fn from(value: &chrono_tz::Tz) -> Self {
    Self {
      time_zone: value.name().to_string(),
    }
  }
}

impl From<chrono_tz::Tz> for Tz {
  fn from(value: chrono_tz::Tz) -> Self {
    Self::from(&value)
  }
}

impl TryFrom<Tz> for chrono_tz::Tz {
  type Error = Error;

  fn try_from(value: Tz) -> std::result::Result<Self, Self::Error> {
    Self::from_str(&value.time_zone).map_err(|_| Error::InvalidTz(value.time_zone))
  }
}

#[cfg(test)]
mod tests {
  use uuid::Uuid as ExternalUuid;

  use super::*;

  #[test]
  fn test_uuid_round_trip() {
    let original = ExternalUuid::new_v4();
    let proto_uuid = Uuid::from(&original);
    let round_tripped: ExternalUuid = (&proto_uuid).into();

    assert_eq!(original, round_tripped, "Round-trip conversion failed!");
  }

  #[test]
  fn test_nil_uuid() {
    let nil = ExternalUuid::nil();
    let proto_uuid = Uuid::from(&nil);

    assert_eq!(proto_uuid.most_significant_bits, 0);
    assert_eq!(proto_uuid.least_significant_bits, 0);
  }

  #[test]
  fn test_specific_bit_alignment() {
    // Using a known UUID: 01234567-89ab-cdef-0123-456789abcdef
    // MSB (0-8 bytes): 01 23 45 67 89 ab cd ef -> 0x0123456789abcdef
    // LSB (8-16 bytes): 01 23 45 67 89 ab cd ef -> 0x0123456789abcdef
    let input_str = "01234567-89ab-cdef-0123-456789abcdef";
    let external = ExternalUuid::parse_str(input_str).unwrap();

    let proto = Uuid::from(&external);

    assert_eq!(proto.most_significant_bits, 0x0123456789abcdef);
    assert_eq!(proto.least_significant_bits, 0x0123456789abcdef);
  }

  #[test]
  fn test_endian_ordering() {
    let external = ExternalUuid::from_u128(0xFF00000000000000_0000000000000000);
    let proto = Uuid::from(&external);

    assert_eq!(proto.most_significant_bits, 0xFF00000000000000);
    assert_eq!(proto.least_significant_bits, 0);
  }
}
