use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenericUid<const T: usize>
where
  [u8; T]: Sized,
{
  pub bytes: [u8; T],
}

impl<const T: usize> GenericUid<T> {
  /// Create a GenericUid from a byte array.
  ///
  /// You shouldn't typically need to use this function as an end-user.
  pub fn new(bytes: [u8; T]) -> Self {
    Self { bytes }
  }

  /// Get the underlying bytes of the UID
  pub fn as_bytes(&self) -> &[u8] {
    &self.bytes
  }
}

/// The unique identifier returned by a PICC
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Uid {
  /// Single sized UID, 4 bytes long
  Single(GenericUid<4>),
  /// Double sized UID, 7 bytes long
  Double(GenericUid<7>),
  /// Triple sized UID, 10 bytes long
  Triple(GenericUid<10>),
}

impl Uid {
  /// Get the UID as a byte slice
  pub fn as_bytes(&self) -> &[u8] {
    match self {
      Uid::Single(u) => u.as_bytes(),
      Uid::Double(u) => u.as_bytes(),
      Uid::Triple(u) => u.as_bytes(),
    }
  }

  /// Get the UID as a hex string
  pub fn as_hex(&self) -> String {
    let mut s = String::new();
    for byte in self.as_bytes() {
      let _ = core::fmt::write(&mut s, format_args!("{:02X}", byte));
    }
    s
  }

  /// Create a UID from a hex string
  pub fn from_hex(s: &str) -> Result<Self> {
    let mut cleaned = String::with_capacity(s.len());
    for c in s.chars() {
      if c == ' ' || c == ';' || c == ':' || c == '.' || c == ',' || c == '-' {
        continue;
      }
      cleaned.push(c);
    }

    let len = cleaned.len();
    if !len.is_multiple_of(2) {
      return Err(Error::UidUnevenLength(len));
    }

    let byte_len = len / 2;

    fn parse<const N: usize>(s: &str) -> Result<[u8; N]> {
      let mut out = [0u8; N];
      for (i, out_byte) in out.iter_mut().enumerate() {
        let idx = i * 2;
        *out_byte = u8::from_str_radix(&s[idx..idx + 2], 16).map_err(|_| Error::InvalidUidHex)?;
      }
      Ok(out)
    }

    match byte_len {
      4 => Ok(Uid::Single(GenericUid::new(parse::<4>(&cleaned)?))),
      7 => Ok(Uid::Double(GenericUid::new(parse::<7>(&cleaned)?))),
      10 => Ok(Uid::Triple(GenericUid::new(parse::<10>(&cleaned)?))),
      _ => Err(Error::InvalidUidLength(byte_len)),
    }
  }
}

impl serde::Serialize for Uid {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_str(&self.as_hex())
  }
}

impl<'de> serde::Deserialize<'de> for Uid {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    Uid::from_hex(&s).map_err(serde::de::Error::custom)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn bytes<const N: usize>(uid: Uid) -> [u8; N] {
    let mut out = [0u8; N];
    out.copy_from_slice(uid.as_bytes());
    out
  }

  #[test]
  fn parse_single_plain() {
    let uid = Uid::from_hex("A1B2C3D4").unwrap();
    assert_eq!(bytes::<4>(uid), [0xA1, 0xB2, 0xC3, 0xD4]);
  }

  #[test]
  fn parse_single_with_spaces() {
    let uid = Uid::from_hex("A1 B2 C3 D4").unwrap();
    assert_eq!(bytes::<4>(uid), [0xA1, 0xB2, 0xC3, 0xD4]);
  }

  #[test]
  fn parse_single_with_semicolons() {
    let uid = Uid::from_hex("A1;B2;C3;D4").unwrap();
    assert_eq!(bytes::<4>(uid), [0xA1, 0xB2, 0xC3, 0xD4]);
  }

  #[test]
  fn parse_single_mixed_separators() {
    let uid = Uid::from_hex("A1; B2 C3;D4").unwrap();
    assert_eq!(bytes::<4>(uid), [0xA1, 0xB2, 0xC3, 0xD4]);
  }

  #[test]
  fn parse_double_uid() {
    let uid = Uid::from_hex("01020304050607").unwrap();
    assert_eq!(bytes::<7>(uid), [1, 2, 3, 4, 5, 6, 7]);
  }

  #[test]
  fn parse_triple_uid_with_noise() {
    let uid = Uid::from_hex("01 02;03 -.,04;05 06;07 08;09 0A").unwrap();
    assert_eq!(bytes::<10>(uid), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
  }

  #[test]
  fn rejects_uneven_length() {
    let err = Uid::from_hex("ABC").unwrap_err();
    matches!(err, Error::UidUnevenLength(_));
  }

  #[test]
  fn rejects_invalid_hex() {
    let err = Uid::from_hex("ZZZZZZZZ").unwrap_err();
    matches!(err, Error::InvalidUidHex);
  }

  #[test]
  fn rejects_invalid_length() {
    // 5 bytes → invalid
    let err = Uid::from_hex("0102030405").unwrap_err();
    matches!(err, Error::InvalidUidLength(5));
  }

  #[test]
  fn empty_input() {
    let err = Uid::from_hex("").unwrap_err();
    matches!(err, Error::InvalidUidLength(0));
  }

  #[test]
  fn separators_only() {
    let err = Uid::from_hex(" ; ;  ").unwrap_err();
    matches!(err, Error::InvalidUidLength(0));
  }

  #[test]
  fn roundtrip_hex() {
    let original = "DE AD BE EF";
    let uid = Uid::from_hex(original).unwrap();
    assert_eq!(uid.as_hex(), "DEADBEEF");
  }

  #[test]
  fn serde_serialize() {
    let uid = Uid::from_hex("A1B2C3D4").unwrap();
    let json = serde_json::to_string(&uid).unwrap();
    assert_eq!(json, "\"A1B2C3D4\"");
  }

  #[test]
  fn serde_deserialize() {
    let json = "\"A1 B2 C3 D4\"";
    let uid: Uid = serde_json::from_str(json).unwrap();
    assert_eq!(uid.as_hex(), "A1B2C3D4");
  }

  #[test]
  fn serde_roundtrip() {
    let uid = Uid::from_hex("01 02 03 04 05 06 07").unwrap();
    let json = serde_json::to_string(&uid).unwrap();
    let back: Uid = serde_json::from_str(&json).unwrap();
    assert_eq!(uid, back);
  }
}
