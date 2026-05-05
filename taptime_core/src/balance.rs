use std::{
  cmp::Ordering::{Equal, Greater, Less},
  ops::{Add, Sub},
};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Balance {
  Overtime(chrono::Duration),
  Exact,
  UnderTime(chrono::Duration),
}

impl Balance {
  pub fn calculate(actual: chrono::Duration, required: chrono::Duration) -> Self {
    let diff = actual - required;
    Self::from_delta(diff)
  }

  pub fn from_delta(delta: chrono::Duration) -> Self {
    if delta > chrono::Duration::zero() {
      Balance::Overtime(delta)
    } else if delta < chrono::Duration::zero() {
      Balance::UnderTime(-delta)
    } else {
      Balance::Exact
    }
  }
}

impl PartialEq for Balance {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Balance::Exact, Balance::Exact) => true,
      (Balance::Exact, Balance::Overtime(dur)) => *dur == chrono::Duration::zero(),
      (Balance::Exact, Balance::UnderTime(dur)) => *dur == chrono::Duration::zero(),
      (Balance::Overtime(dur), Balance::Exact) => *dur == chrono::Duration::zero(),
      (Balance::UnderTime(dur), Balance::Exact) => *dur == chrono::Duration::zero(),
      (Balance::Overtime(lhs), Balance::Overtime(rhs)) => lhs == rhs,
      (Balance::UnderTime(lhs), Balance::UnderTime(rhs)) => lhs == rhs,
      _ => false,
    }
  }
}

impl Eq for Balance {}

impl Add for Balance {
  type Output = Self;

  fn add(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Balance::Overtime(lhs_dur), Balance::Overtime(rhs_dur)) => {
        Balance::Overtime(lhs_dur + rhs_dur)
      }
      (Balance::UnderTime(lhs_dur), Balance::UnderTime(rhs_dur)) => {
        Balance::UnderTime(lhs_dur + rhs_dur)
      }
      (Balance::Overtime(lhs_dur), Balance::UnderTime(rhs_dur)) => match lhs_dur.cmp(&rhs_dur) {
        Greater => Balance::Overtime(lhs_dur - rhs_dur),
        Equal => Balance::Exact,
        Less => Balance::UnderTime(rhs_dur - lhs_dur),
      },
      (Balance::UnderTime(lhs_dur), Balance::Overtime(rhs_dur)) => match lhs_dur.cmp(&rhs_dur) {
        Greater => Balance::UnderTime(lhs_dur - rhs_dur),
        Equal => Balance::Exact,
        Less => Balance::Overtime(rhs_dur - lhs_dur),
      },
      (Balance::Exact, other) | (other, Balance::Exact) => other,
    }
  }
}

impl Sub for Balance {
  type Output = Self;

  fn sub(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Balance::Overtime(lhs_dur), Balance::Overtime(rhs_dur)) => match lhs_dur.cmp(&rhs_dur) {
        Greater => Balance::Overtime(lhs_dur - rhs_dur),
        Equal => Balance::Exact,
        Less => Balance::UnderTime(rhs_dur - lhs_dur),
      },
      (Balance::Overtime(lhs_dur), Balance::UnderTime(rhs_dur)) => {
        Balance::Overtime(lhs_dur + rhs_dur)
      }
      (Balance::UnderTime(lhs_dur), Balance::Overtime(rhs_dur)) => {
        Balance::UnderTime(lhs_dur + rhs_dur)
      }
      (Balance::UnderTime(lhs_dur), Balance::UnderTime(rhs_dur)) => match lhs_dur.cmp(&rhs_dur) {
        Greater => Balance::UnderTime(lhs_dur - rhs_dur),
        Equal => Balance::Exact,
        Less => Balance::Overtime(rhs_dur - lhs_dur),
      },
      (Balance::Exact, Balance::Overtime(rhs_dur)) => Balance::UnderTime(rhs_dur),
      (Balance::Exact, Balance::UnderTime(rhs_dur)) => Balance::Overtime(rhs_dur),
      (Balance::Exact, Balance::Exact) => Balance::Exact,
      (other, Balance::Exact) => other,
    }
  }
}

impl Add<chrono::Duration> for Balance {
  type Output = Self;

  fn add(self, rhs: chrono::Duration) -> Self::Output {
    let delta = match self {
      Balance::Overtime(dur) => dur,
      Balance::UnderTime(dur) => -dur,
      Balance::Exact => chrono::Duration::zero(),
    };

    Self::from_delta(delta + rhs)
  }
}

impl Sub<chrono::Duration> for Balance {
  type Output = Self;

  fn sub(self, rhs: chrono::Duration) -> Self::Output {
    self + (-rhs)
  }
}

#[cfg(test)]
mod tests {
  use chrono::Duration;

  use super::*;

  #[test]
  fn test_add_overtime_to_overtime() {
    let ot1 = Balance::Overtime(Duration::hours(2));
    let ot2 = Balance::Overtime(Duration::hours(3));
    assert_eq!(ot1 + ot2, Balance::Overtime(Duration::hours(5)));
  }

  #[test]
  fn test_add_undertime_to_undertime() {
    let ut1 = Balance::UnderTime(Duration::hours(1));
    let ut2 = Balance::UnderTime(Duration::hours(4));
    assert_eq!(ut1 + ut2, Balance::UnderTime(Duration::hours(5)));
  }

  #[test]
  fn test_add_overtime_and_undertime_cancels_to_exact() {
    let ot = Balance::Overtime(Duration::hours(3));
    let ut = Balance::UnderTime(Duration::hours(3));
    assert_eq!(ot + ut, Balance::Exact);
    assert_eq!(ut + ot, Balance::Exact);
  }

  #[test]
  fn test_add_overtime_and_undertime_net_overtime() {
    let ot = Balance::Overtime(Duration::hours(5));
    let ut = Balance::UnderTime(Duration::hours(2));
    assert_eq!(ot + ut, Balance::Overtime(Duration::hours(3)));
  }

  #[test]
  fn test_add_overtime_and_undertime_net_undertime() {
    let ot = Balance::Overtime(Duration::hours(2));
    let ut = Balance::UnderTime(Duration::hours(5));
    assert_eq!(ot + ut, Balance::UnderTime(Duration::hours(3)));
    assert_eq!(ut + ot, Balance::UnderTime(Duration::hours(3)));
  }

  #[test]
  fn test_add_exact_with_anything() {
    let exact = Balance::Exact;
    let ot = Balance::Overtime(Duration::hours(4));
    let ut = Balance::UnderTime(Duration::hours(2));

    assert_eq!(exact + ot, Balance::Overtime(Duration::hours(4)));
    assert_eq!(ot + exact, Balance::Overtime(Duration::hours(4)));
    assert_eq!(exact + ut, Balance::UnderTime(Duration::hours(2)));
    assert_eq!(ut + exact, Balance::UnderTime(Duration::hours(2)));
  }

  #[test]
  fn test_sub_overtime_minus_overtime_net_overtime() {
    let ot1 = Balance::Overtime(Duration::hours(5));
    let ot2 = Balance::Overtime(Duration::hours(2));
    assert_eq!(ot1 - ot2, Balance::Overtime(Duration::hours(3)));
  }

  #[test]
  fn test_sub_overtime_minus_overtime_exact() {
    let ot1 = Balance::Overtime(Duration::hours(3));
    let ot2 = Balance::Overtime(Duration::hours(3));
    assert_eq!(ot1 - ot2, Balance::Exact);
  }

  #[test]
  fn test_sub_overtime_minus_overtime_net_undertime() {
    let ot1 = Balance::Overtime(Duration::hours(2));
    let ot2 = Balance::Overtime(Duration::hours(5));
    assert_eq!(ot1 - ot2, Balance::UnderTime(Duration::hours(3)));
  }

  #[test]
  fn test_sub_undertime_minus_undertime_net_undertime() {
    let ut1 = Balance::UnderTime(Duration::hours(5));
    let ut2 = Balance::UnderTime(Duration::hours(2));
    assert_eq!(ut1 - ut2, Balance::UnderTime(Duration::hours(3)));
  }

  #[test]
  fn test_sub_undertime_minus_undertime_exact() {
    let ut1 = Balance::UnderTime(Duration::hours(3));
    let ut2 = Balance::UnderTime(Duration::hours(3));
    assert_eq!(ut1 - ut2, Balance::Exact);
  }

  #[test]
  fn test_sub_undertime_minus_undertime_net_overtime() {
    let ut1 = Balance::UnderTime(Duration::hours(2));
    let ut2 = Balance::UnderTime(Duration::hours(5));
    assert_eq!(ut1 - ut2, Balance::Overtime(Duration::hours(3)));
  }

  #[test]
  fn test_sub_overtime_minus_undertime() {
    let ot = Balance::Overtime(Duration::hours(3));
    let ut = Balance::UnderTime(Duration::hours(2));
    // Overtime(3) - Undertime(2) = Overtime(5) because subtracting negative adds
    assert_eq!(ot - ut, Balance::Overtime(Duration::hours(5)));
  }

  #[test]
  fn test_sub_undertime_minus_overtime() {
    let ut = Balance::UnderTime(Duration::hours(3));
    let ot = Balance::Overtime(Duration::hours(2));
    // Undertime(3) - Overtime(2) = Undertime(5) because subtracting positive adds more negative
    assert_eq!(ut - ot, Balance::UnderTime(Duration::hours(5)));
  }

  #[test]
  fn test_sub_exact_minus_something() {
    let exact = Balance::Exact;
    let ot = Balance::Overtime(Duration::hours(4));
    let ut = Balance::UnderTime(Duration::hours(2));

    assert_eq!(exact - ot, Balance::UnderTime(Duration::hours(4)));
    assert_eq!(exact - ut, Balance::Overtime(Duration::hours(2)));
  }

  #[test]
  fn test_sub_something_minus_exact() {
    let exact = Balance::Exact;
    let ot = Balance::Overtime(Duration::hours(4));
    let ut = Balance::UnderTime(Duration::hours(2));

    assert_eq!(ot - exact, Balance::Overtime(Duration::hours(4)));
    assert_eq!(ut - exact, Balance::UnderTime(Duration::hours(2)));
  }

  #[test]
  fn test_add_duration_to_balance() {
    let ot = Balance::Overtime(Duration::hours(3));
    let ut = Balance::UnderTime(Duration::hours(2));
    let exact = Balance::Exact;

    assert_eq!(
      ot + Duration::hours(2),
      Balance::Overtime(Duration::hours(5))
    );
    assert_eq!(
      ot + Duration::hours(-1),
      Balance::Overtime(Duration::hours(2))
    );
    assert_eq!(ot + Duration::hours(-3), Balance::Exact);
    assert_eq!(
      ot + Duration::hours(-5),
      Balance::UnderTime(Duration::hours(2))
    );

    assert_eq!(
      ut + Duration::hours(1),
      Balance::UnderTime(Duration::hours(1))
    );
    assert_eq!(ut + Duration::hours(2), Balance::Exact);
    assert_eq!(
      ut + Duration::hours(3),
      Balance::Overtime(Duration::hours(1))
    );
    assert_eq!(
      ut + Duration::hours(-1),
      Balance::UnderTime(Duration::hours(3))
    );

    assert_eq!(
      exact + Duration::hours(2),
      Balance::Overtime(Duration::hours(2))
    );
    assert_eq!(
      exact + Duration::hours(-2),
      Balance::UnderTime(Duration::hours(2))
    );
    assert_eq!(exact + Duration::hours(0), Balance::Exact);
  }

  #[test]
  fn test_sub_duration_from_balance() {
    let ot = Balance::Overtime(Duration::hours(3));
    let ut = Balance::UnderTime(Duration::hours(2));
    let exact = Balance::Exact;

    assert_eq!(
      ot - Duration::hours(1),
      Balance::Overtime(Duration::hours(2))
    );
    assert_eq!(ot - Duration::hours(3), Balance::Exact);
    assert_eq!(
      ot - Duration::hours(5),
      Balance::UnderTime(Duration::hours(2))
    );
    assert_eq!(
      ot - Duration::hours(-2),
      Balance::Overtime(Duration::hours(5))
    );

    assert_eq!(
      ut - Duration::hours(1),
      Balance::UnderTime(Duration::hours(3))
    );
    assert_eq!(ut - Duration::hours(-2), Balance::Exact);
    assert_eq!(
      ut - Duration::hours(-3),
      Balance::Overtime(Duration::hours(1))
    );

    assert_eq!(
      exact - Duration::hours(2),
      Balance::UnderTime(Duration::hours(2))
    );
    assert_eq!(
      exact - Duration::hours(-2),
      Balance::Overtime(Duration::hours(2))
    );
    assert_eq!(exact - Duration::hours(0), Balance::Exact);
  }

  #[test]
  fn test_commutative_property_for_addition() {
    let a = Balance::Overtime(Duration::hours(2));
    let b = Balance::UnderTime(Duration::hours(1));
    assert_eq!(a + b, b + a);

    let c = Balance::Exact;
    assert_eq!(a + c, c + a);
    assert_eq!(b + c, c + b);
  }

  #[test]
  fn test_associative_property_for_addition() {
    let a = Balance::Overtime(Duration::hours(3));
    let b = Balance::UnderTime(Duration::hours(1));
    let c = Balance::Overtime(Duration::hours(2));

    let ab = a + b;
    let result1 = ab + c;
    let bc = b + c;
    let result2 = a + bc;

    assert_eq!(result1, result2);
  }

  #[test]
  fn test_zero_duration_operations() {
    let zero_dur = Duration::zero();

    assert_eq!(Balance::Exact + zero_dur, Balance::Exact);
    assert_eq!(Balance::Exact - zero_dur, Balance::Exact);

    assert_eq!(Balance::Overtime(zero_dur) + Balance::Exact, Balance::Exact);
    assert_eq!(
      Balance::UnderTime(zero_dur) + Balance::Exact,
      Balance::Exact
    );

    assert_eq!(
      Balance::Overtime(zero_dur) - Balance::Overtime(zero_dur),
      Balance::Exact
    );
    assert_eq!(
      Balance::UnderTime(zero_dur) - Balance::UnderTime(zero_dur),
      Balance::Exact
    );
  }
}
