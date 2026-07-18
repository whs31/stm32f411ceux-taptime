use std::{
  net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
  str::FromStr,
};

use tonic::{Request, Status};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct AccessConfig {
  pub trust_proxy_headers: bool,
}

#[derive(sqlx::FromRow)]
struct IpBanRow {
  ip_cidr: String,
  reason: String,
}

pub fn client_ip<T>(request: &Request<T>, config: AccessConfig) -> Option<IpAddr> {
  if config.trust_proxy_headers
    && let Some(ip) = forwarded_ip(request)
  {
    return Some(ip);
  }
  request.remote_addr().map(|addr| addr.ip())
}

pub fn normalize_ip_cidr(value: &str) -> Result<String, Status> {
  let value = value.trim();
  if value.is_empty() {
    return Err(Status::invalid_argument("IP/CIDR cannot be empty"));
  }
  if value.contains('/') {
    parse_cidr(value)
      .map(|cidr| cidr.to_string())
      .ok_or_else(|| Status::invalid_argument("Invalid IP/CIDR"))
  } else {
    value
      .parse::<IpAddr>()
      .map(|ip| ip.to_string())
      .map_err(|_| Status::invalid_argument("Invalid IP address"))
  }
}

pub fn ip_matches_cidr(ip: IpAddr, cidr: &str) -> bool {
  if cidr.contains('/') {
    parse_cidr(cidr)
      .map(|cidr| cidr.contains(ip))
      .unwrap_or(false)
  } else {
    cidr
      .parse::<IpAddr>()
      .map(|ban_ip| ban_ip == ip)
      .unwrap_or(false)
  }
}

pub async fn enforce_ip_allowed(
  db: &sqlx::PgPool,
  ip: Option<IpAddr>,
) -> Result<Option<IpAddr>, Status> {
  let Some(ip) = ip else {
    return Ok(None);
  };

  let rows = sqlx::query_as::<_, IpBanRow>(include_str!("queries/fetch_active_ip_bans.sql"))
    .fetch_all(db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?;
  for row in rows {
    if ip_matches_cidr(ip, &row.ip_cidr) {
      return Err(Status::permission_denied(ban_message(
        "IP address banned",
        &row.reason,
      )));
    }
  }
  Ok(Some(ip))
}

pub async fn enforce_user_allowed(db: &sqlx::PgPool, user_id: Uuid) -> Result<(), Status> {
  let reason: Option<String> =
    sqlx::query_scalar(include_str!("queries/fetch_active_user_ban.sql"))
      .bind(user_id)
      .fetch_optional(db)
      .await
      .map_err(|e| Status::internal(e.to_string()))?;
  if let Some(reason) = reason {
    return Err(Status::permission_denied(ban_message(
      "Account banned",
      &reason,
    )));
  }
  Ok(())
}

pub async fn record_user_ip(
  db: &sqlx::PgPool,
  user_id: Uuid,
  ip: Option<IpAddr>,
) -> Result<(), Status> {
  let Some(ip) = ip else {
    return Ok(());
  };
  sqlx::query(include_str!("queries/upsert_user_ip_address.sql"))
    .bind(user_id)
    .bind(ip.to_string())
    .execute(db)
    .await
    .map_err(|e| Status::internal(e.to_string()))?;
  Ok(())
}

fn forwarded_ip<T>(request: &Request<T>) -> Option<IpAddr> {
  metadata_value(request, "x-forwarded-for")
    .and_then(|value| value.split(',').find_map(parse_ip_candidate))
    .or_else(|| metadata_value(request, "x-real-ip").and_then(parse_ip_candidate))
}

fn metadata_value<'a, T>(request: &'a Request<T>, key: &str) -> Option<&'a str> {
  request.metadata().get(key)?.to_str().ok()
}

fn parse_ip_candidate(value: &str) -> Option<IpAddr> {
  let value = value.trim().trim_matches('"');
  if value.is_empty() {
    return None;
  }
  if let Ok(ip) = IpAddr::from_str(value) {
    return Some(ip);
  }
  if let Ok(socket) = SocketAddr::from_str(value) {
    return Some(socket.ip());
  }
  if let Some(stripped) = value.strip_prefix('[').and_then(|v| v.split(']').next()) {
    return IpAddr::from_str(stripped).ok();
  }
  None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cidr {
  V4(Ipv4Addr, u8),
  V6(Ipv6Addr, u8),
}

impl Cidr {
  fn contains(self, ip: IpAddr) -> bool {
    match (self, ip) {
      (Self::V4(network, prefix), IpAddr::V4(ip)) => {
        let mask = prefix_mask_v4(prefix);
        u32::from(network) & mask == u32::from(ip) & mask
      }
      (Self::V6(network, prefix), IpAddr::V6(ip)) => {
        let mask = prefix_mask_v6(prefix);
        u128::from(network) & mask == u128::from(ip) & mask
      }
      _ => false,
    }
  }
}

impl std::fmt::Display for Cidr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::V4(ip, prefix) => write!(f, "{ip}/{prefix}"),
      Self::V6(ip, prefix) => write!(f, "{ip}/{prefix}"),
    }
  }
}

fn parse_cidr(value: &str) -> Option<Cidr> {
  let (ip, prefix) = value.trim().split_once('/')?;
  let prefix = prefix.parse::<u8>().ok()?;
  match ip.parse::<IpAddr>().ok()? {
    IpAddr::V4(ip) if prefix <= 32 => Some(Cidr::V4(ip, prefix)),
    IpAddr::V6(ip) if prefix <= 128 => Some(Cidr::V6(ip, prefix)),
    _ => None,
  }
}

fn prefix_mask_v4(prefix: u8) -> u32 {
  if prefix == 0 {
    0
  } else {
    u32::MAX << (32 - prefix)
  }
}

fn prefix_mask_v6(prefix: u8) -> u128 {
  if prefix == 0 {
    0
  } else {
    u128::MAX << (128 - prefix)
  }
}

fn ban_message(prefix: &str, reason: &str) -> String {
  let reason = reason.trim();
  if reason.is_empty() {
    prefix.to_string()
  } else {
    format!("{prefix}: {reason}")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normalizes_ip_and_cidr() {
    assert_eq!(normalize_ip_cidr("127.0.0.1").unwrap(), "127.0.0.1");
    assert_eq!(normalize_ip_cidr("10.0.0.0/24").unwrap(), "10.0.0.0/24");
    assert!(normalize_ip_cidr("nope").is_err());
  }

  #[test]
  fn cidr_matching_handles_single_ips_and_networks() {
    let ip: IpAddr = "10.0.0.42".parse().unwrap();
    assert!(ip_matches_cidr(ip, "10.0.0.0/24"));
    assert!(ip_matches_cidr(ip, "10.0.0.42"));
    assert!(!ip_matches_cidr(ip, "10.0.1.0/24"));
  }

  #[test]
  fn parses_forwarded_ip_candidates() {
    assert_eq!(
      parse_ip_candidate("203.0.113.7"),
      Some("203.0.113.7".parse().unwrap())
    );
    assert_eq!(
      parse_ip_candidate("203.0.113.7:443"),
      Some("203.0.113.7".parse().unwrap())
    );
    assert_eq!(
      parse_ip_candidate("[2001:db8::1]:443"),
      Some("2001:db8::1".parse().unwrap())
    );
  }
}
