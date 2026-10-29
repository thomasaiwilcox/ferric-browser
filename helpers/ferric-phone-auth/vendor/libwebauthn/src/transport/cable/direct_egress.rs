//! Ferric's deliberately narrow first hybrid route. It neither implements a
//! proxy nor follows redirects. DNS answers are checked once and the selected
//! socket address is used directly, avoiding a second DNS lookup at connect.
//! Address exclusions follow IANA's special-purpose registries (2025-10-09).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::{client_async_tls, MaybeTlsStream, WebSocketStream};
use url::Url;

pub fn relay_domain_allowed(host: &str) -> bool {
    matches!(host, "cable.ua5v.com" | "cable.auth.com")
}

fn public_v4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 192 && b == 88 && c == 99)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113))
}

fn public_v6(address: Ipv6Addr) -> bool {
    if let Some(v4) = address.to_ipv4_mapped() {
        return public_v4(v4);
    }
    let parts = address.segments();
    (parts[0] & 0xe000) == 0x2000
        && !(parts[0] == 0x2001 && (parts[1] <= 0x01ff || parts[1] == 0x0db8))
        && parts[0] != 0x2002
        && !(parts[0] == 0x3fff && (parts[1] & 0xf000) == 0)
}

pub fn public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => public_v6(v6),
    }
}

pub fn relay_url_allowed(url: &Url) -> bool {
    url.scheme() == "wss"
        && url.host_str().is_some_and(relay_domain_allowed)
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.query().is_none()
}

pub async fn connect(
    request: Request,
) -> Result<
    (
        WebSocketStream<MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ),
    Error,
> {
    let url = Url::parse(&request.uri().to_string()).map_err(|_| denied())?;
    if !relay_url_allowed(&url) {
        return Err(denied());
    }
    let addresses: Vec<_> = timeout(
        Duration::from_secs(10),
        tokio::net::lookup_host((url.host_str().ok_or_else(denied)?, 443)),
    )
    .await
    .map_err(|_| denied())??
    .take(17)
    .collect();
    if addresses.is_empty()
        || addresses.len() > 16
        || addresses
            .iter()
            .any(|address| !public_address(address.ip()))
    {
        return Err(denied());
    }
    for address in addresses {
        if let Ok(Ok(stream)) = timeout(Duration::from_secs(5), TcpStream::connect(address)).await {
            // SNI and normal certificate verification use the original URI;
            // only the already validated TCP destination is supplied here.
            return client_async_tls(request, stream).await;
        }
    }
    Err(denied())
}

fn denied() -> Error {
    Error::Io(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "relay route unavailable",
    ))
}
