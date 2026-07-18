use std::{fmt, net::IpAddr};

use url::{Host, Url};

const SERVICE: &str = "com.whs31.taptime.admin";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialEndpoint {
    endpoint: String,
    cache_allowed: bool,
}

impl CredentialEndpoint {
    pub fn parse(raw: &str) -> Result<Self, EndpointError> {
        let mut url = Url::parse(raw).map_err(|error| EndpointError(error.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(EndpointError(
                "admin API URL must use http or https".to_string(),
            ));
        }
        if url.host().is_none() {
            return Err(EndpointError(
                "admin API URL must include a host".to_string(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(EndpointError(
                "admin API URL must not contain credentials".to_string(),
            ));
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(EndpointError(
                "admin API URL must not contain a query or fragment".to_string(),
            ));
        }

        let default_port = match url.scheme() {
            "http" => Some(80),
            "https" => Some(443),
            _ => None,
        };
        if url.port() == default_port {
            url.set_port(None)
                .map_err(|_| EndpointError("invalid admin API port".to_string()))?;
        }

        let cache_allowed =
            url.scheme() == "https" || (url.scheme() == "http" && host_is_loopback(&url));
        let root_path = url.path() == "/";
        let mut endpoint = url.to_string();
        if root_path {
            endpoint.truncate(endpoint.trim_end_matches('/').len());
        }

        Ok(Self {
            endpoint,
            cache_allowed,
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn account(&self) -> &str {
        &self.endpoint
    }

    pub fn cache_allowed(&self) -> bool {
        self.cache_allowed
    }
}

fn host_is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address == IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        None => false,
    }
}

#[derive(Debug)]
pub struct EndpointError(String);

impl fmt::Display for EndpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Invalid admin API URL: {}", self.0)
    }
}

impl std::error::Error for EndpointError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CredentialError {
    NotFound,
    Unavailable(String),
}

impl fmt::Display for CredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("credential not found"),
            Self::Unavailable(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CredentialError {}

pub trait CredentialStore {
    fn get_password(&mut self, account: &str) -> Result<String, CredentialError>;
    fn set_password(&mut self, account: &str, password: &str) -> Result<(), CredentialError>;
    fn delete_password(&mut self, account: &str) -> Result<(), CredentialError>;
}

#[derive(Default)]
pub struct SystemCredentialStore;

impl SystemCredentialStore {
    fn entry(account: &str) -> Result<keyring::Entry, CredentialError> {
        keyring::Entry::new(SERVICE, account).map_err(map_keyring_error)
    }
}

impl CredentialStore for SystemCredentialStore {
    fn get_password(&mut self, account: &str) -> Result<String, CredentialError> {
        Self::entry(account)?
            .get_password()
            .map_err(map_keyring_error)
    }

    fn set_password(&mut self, account: &str, password: &str) -> Result<(), CredentialError> {
        Self::entry(account)?
            .set_password(password)
            .map_err(map_keyring_error)
    }

    fn delete_password(&mut self, account: &str) -> Result<(), CredentialError> {
        Self::entry(account)?
            .delete_credential()
            .map_err(map_keyring_error)
    }
}

fn map_keyring_error(error: keyring::Error) -> CredentialError {
    match error {
        keyring::Error::NoEntry => CredentialError::NotFound,
        error => CredentialError::Unavailable(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_equivalent_endpoints() {
        let upper = CredentialEndpoint::parse("HTTPS://Example.COM:443/").unwrap();
        let lower = CredentialEndpoint::parse("https://example.com").unwrap();
        assert_eq!(upper.endpoint(), "https://example.com");
        assert_eq!(upper, lower);
    }

    #[test]
    fn preserves_distinct_ports_and_paths() {
        let base = CredentialEndpoint::parse("https://example.com").unwrap();
        let port = CredentialEndpoint::parse("https://example.com:8443").unwrap();
        let path = CredentialEndpoint::parse("https://example.com/admin").unwrap();
        assert_ne!(base.account(), port.account());
        assert_ne!(base.account(), path.account());
    }

    #[test]
    fn permits_https_and_loopback_http_only() {
        for endpoint in [
            "https://example.com",
            "http://localhost:50051",
            "http://127.42.0.1:50051",
            "http://[::1]:50051",
        ] {
            assert!(CredentialEndpoint::parse(endpoint).unwrap().cache_allowed());
        }
        assert!(
            !CredentialEndpoint::parse("http://server:50051")
                .unwrap()
                .cache_allowed()
        );
        assert!(
            !CredentialEndpoint::parse("http://192.168.1.10:50051")
                .unwrap()
                .cache_allowed()
        );
    }

    #[test]
    fn rejects_ambiguous_or_unsafe_endpoint_shapes() {
        for endpoint in [
            "ftp://example.com",
            "https://user:pass@example.com",
            "https://example.com?target=other",
            "https://example.com#fragment",
        ] {
            assert!(CredentialEndpoint::parse(endpoint).is_err());
        }
    }

    #[test]
    #[ignore = "uses the logged-in user's native credential store"]
    fn native_credential_store_round_trip() {
        let account = format!("https://credential-smoke-{}.invalid", uuid::Uuid::new_v4());
        let password = format!("smoke-{}", uuid::Uuid::new_v4());
        let mut store = SystemCredentialStore;

        struct Cleanup<'a> {
            store: &'a mut SystemCredentialStore,
            account: String,
        }
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.store.delete_password(&self.account);
            }
        }

        store.set_password(&account, &password).unwrap();
        let cleanup = Cleanup {
            store: &mut store,
            account,
        };
        assert_eq!(
            cleanup.store.get_password(&cleanup.account).unwrap(),
            password
        );
        cleanup.store.delete_password(&cleanup.account).unwrap();
        assert_eq!(
            cleanup.store.get_password(&cleanup.account),
            Err(CredentialError::NotFound)
        );
    }
}
