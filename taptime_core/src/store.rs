use std::collections::BTreeMap;

use chrono::NaiveDate;
use uuid::Uuid;

use crate::{Day, Result};

pub trait DayStore {
  fn get(&self, user_id: Uuid, date: NaiveDate) -> Result<Option<Day>>;
  fn save(&mut self, user_id: Uuid, day: Day) -> Result<()>;
  fn range(&self, user_id: Uuid, from: NaiveDate, to: NaiveDate) -> Result<Vec<Day>>;
  fn delete(&mut self, user_id: Uuid, date: NaiveDate) -> Result<bool>;
}

#[derive(Debug, Default)]
pub struct MemoryDayStore {
  days: BTreeMap<(Uuid, NaiveDate), Day>,
}

impl MemoryDayStore {
  pub fn new() -> Self {
    Self::default()
  }
}

impl DayStore for MemoryDayStore {
  fn get(&self, user_id: Uuid, date: NaiveDate) -> Result<Option<Day>> {
    Ok(self.days.get(&(user_id, date)).cloned())
  }

  fn save(&mut self, user_id: Uuid, day: Day) -> Result<()> {
    self.days.insert((user_id, day.date), day);
    Ok(())
  }

  fn range(&self, user_id: Uuid, from: NaiveDate, to: NaiveDate) -> Result<Vec<Day>> {
    let days = self
      .days
      .range((user_id, from)..=(user_id, to))
      .map(|(_, day)| day.clone())
      .collect();
    Ok(days)
  }

  fn delete(&mut self, user_id: Uuid, date: NaiveDate) -> Result<bool> {
    Ok(self.days.remove(&(user_id, date)).is_some())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::DayFlags;

  fn uid() -> Uuid {
    Uuid::new_v4()
  }

  fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
  }

  fn empty_day(d: NaiveDate) -> Day {
    Day {
      date: d,
      events: vec![],
      flags: DayFlags::empty(),
      required_work_hours: chrono::Duration::hours(8),
      lunch_break_duration: chrono::Duration::minutes(30),
    }
  }

  #[test]
  fn get_missing_returns_none() {
    let store = MemoryDayStore::new();
    assert!(store.get(uid(), date(2024, 1, 1)).unwrap().is_none());
  }

  #[test]
  fn save_then_get_roundtrip() {
    let mut store = MemoryDayStore::new();
    let user = uid();
    let d = date(2024, 6, 15);
    store.save(user, empty_day(d)).unwrap();
    assert_eq!(store.get(user, d).unwrap().unwrap().date, d);
  }

  #[test]
  fn save_overwrites_existing_entry() {
    let mut store = MemoryDayStore::new();
    let user = uid();
    let d = date(2024, 6, 15);
    store.save(user, empty_day(d)).unwrap();
    let mut updated = empty_day(d);
    updated.set_weekend();
    store.save(user, updated).unwrap();
    assert!(store.get(user, d).unwrap().unwrap().is_weekend());
  }

  #[test]
  fn delete_existing_returns_true_and_removes() {
    let mut store = MemoryDayStore::new();
    let user = uid();
    let d = date(2024, 6, 15);
    store.save(user, empty_day(d)).unwrap();
    assert!(store.delete(user, d).unwrap());
    assert!(store.get(user, d).unwrap().is_none());
  }

  #[test]
  fn delete_missing_returns_false() {
    let mut store = MemoryDayStore::new();
    assert!(!store.delete(uid(), date(2024, 6, 15)).unwrap());
  }

  #[test]
  fn range_returns_days_within_inclusive_bounds() {
    let mut store = MemoryDayStore::new();
    let user = uid();
    for d in [
      date(2024, 6, 1),
      date(2024, 6, 10),
      date(2024, 6, 20),
      date(2024, 6, 30),
    ] {
      store.save(user, empty_day(d)).unwrap();
    }
    let result = store.range(user, date(2024, 6, 10), date(2024, 6, 20)).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].date, date(2024, 6, 10));
    assert_eq!(result[1].date, date(2024, 6, 20));
  }

  #[test]
  fn range_is_empty_when_no_days_stored() {
    let store = MemoryDayStore::new();
    assert!(store
      .range(uid(), date(2024, 6, 1), date(2024, 6, 30))
      .unwrap()
      .is_empty());
  }

  #[test]
  fn range_excludes_other_users() {
    let mut store = MemoryDayStore::new();
    let user_a = uid();
    let user_b = uid();
    let d = date(2024, 6, 15);
    store.save(user_a, empty_day(d)).unwrap();
    store.save(user_b, empty_day(d)).unwrap();
    let result = store
      .range(user_a, date(2024, 6, 1), date(2024, 6, 30))
      .unwrap();
    assert_eq!(result.len(), 1);
  }

  #[test]
  fn range_returns_sorted_by_date() {
    let mut store = MemoryDayStore::new();
    let user = uid();
    // Insert out of order
    store.save(user, empty_day(date(2024, 6, 20))).unwrap();
    store.save(user, empty_day(date(2024, 6, 5))).unwrap();
    store.save(user, empty_day(date(2024, 6, 12))).unwrap();
    let result = store
      .range(user, date(2024, 6, 1), date(2024, 6, 30))
      .unwrap();
    assert_eq!(result[0].date, date(2024, 6, 5));
    assert_eq!(result[1].date, date(2024, 6, 12));
    assert_eq!(result[2].date, date(2024, 6, 20));
  }

  // Smoke-test that the trait object compiles (dyn DayStore is object-safe).
  #[test]
  fn trait_is_object_safe() {
    let store: Box<dyn DayStore> = Box::new(MemoryDayStore::new());
    drop(store);
  }
}
