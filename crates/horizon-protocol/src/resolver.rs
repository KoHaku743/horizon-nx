//! Minecraft server DNS redirects (`ServerRedirectHandler` / `ServerNameResolver`).
//! Port 25565 asks for `_minecraft._tcp.<host>`'s SRV record; no record or a
//! failed query leaves the original address in use. Login uses the resolved
//! host and port; status requests keep the original address.
use crate::{Error, Result};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

mod dns;
pub use dns::set_name_servers;

#[derive(Debug)]
pub(crate) struct Resolved {
    pub host: String,
    pub port: u16,
    pub addresses: Vec<SocketAddr>,
}

pub(crate) fn resolve(host: &str, port: u16) -> Result<Resolved> {
    resolve_with(host, port, srv_redirect, |host, port| {
        (host, port)
            .to_socket_addrs()
            .map(|addrs| addrs.collect())
            .map_err(|e| Error::new(format!("Unknown host {host}: {e}")))
    })
}

/// `ServerNameResolver.resolveAddress`: keep the selected host and port with
/// its socket addresses, including a single SRV redirect.
fn resolve_with(
    host: &str,
    port: u16,
    srv: impl FnOnce(&str) -> Option<(String, u16)>,
    addresses: impl FnOnce(&str, u16) -> Result<Vec<SocketAddr>>,
) -> Result<Resolved> {
    // Vanilla also asks for `_minecraft._tcp.127.0.0.1`, which never has a
    // record: IP literals and localhost skip the query.
    if port == 25565
        && host.parse::<IpAddr>().is_err()
        && host != "localhost"
        && let Some((target, target_port)) =
            srv(&format!("_minecraft._tcp.{}.", host.trim_end_matches('.')))
    {
        // No second SRV lookup at the target, and no fallback to the original
        // host when the target is unknown.
        return Ok(Resolved {
            addresses: addresses(&target, target_port)?,
            host: target,
            port: target_port,
        });
    }
    Ok(Resolved {
        addresses: addresses(host, port)?,
        host: host.to_owned(),
        port,
    })
}

/// Where a server address leads (`ServerRedirectHandler`): its `_minecraft._tcp` SRV target when
/// the port is 25565 and the host a name, else the host and port as given. No address lookup.
pub fn server_redirect(host: &str, port: u16) -> (String, u16) {
    resolve_with(host, port, srv_redirect, |_, _| Ok(Vec::new()))
        .map_or_else(|_| (host.to_owned(), port), |r| (r.host, r.port))
}

/// The SRV question on a thread of its own, given up after a deadline: a lookup
/// stuck inside the platform's network stack (the Switch's froze the console,
/// #3) costs a few seconds and the original address, never the join itself.
fn srv_redirect(name: &str) -> Option<(String, u16)> {
    let servers = dns::system_name_servers();
    if servers.is_empty() {
        return None;
    }
    // Each server gets a UDP try and, when truncated, a TCP connect and read.
    let deadline = dns::SERVER_TIMEOUT * (3 * servers.len() as u32) + dns::SERVER_TIMEOUT;
    let (tx, rx) = std::sync::mpsc::channel();
    let owned = name.to_owned();
    let spawned = std::thread::Builder::new()
        .name("SRV lookup".into())
        .stack_size(128 * 1024)
        .spawn(move || {
            let _ = tx.send(dns::query_srv(&owned, &servers, dns::SERVER_TIMEOUT));
        });
    if let Err(e) = spawned {
        log::warn!("SRV lookup for {name} not started ({e}): joining the address as given");
        return None;
    }
    match rx.recv_timeout(deadline) {
        Ok(srv) => srv,
        Err(_) => {
            log::warn!(
                "SRV lookup for {name} gave no answer in {deadline:?}: joining the address as given"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compared with ServerNameResolver by tools/test_server_address_parity.py.
    #[test]
    fn login_handshake_address_matches_vanilla() {
        for (scenario, port, redirected) in [
            ("direct", 25565, false),
            ("redirect", 25565, true),
            ("explicit", 25566, false),
        ] {
            let result = resolve_with(
                "play.example.org",
                port,
                |_| redirected.then(|| ("backend.example.org.".into(), 25570)),
                |_, port| Ok(vec![SocketAddr::from(([127, 0, 0, 1], port))]),
            )
            .unwrap();
            let expected = if redirected {
                ("backend.example.org.", 25570)
            } else {
                ("play.example.org", port)
            };
            assert_eq!((result.host.as_str(), result.port), expected);
            assert_eq!(result.addresses[0].port(), expected.1);
            println!("server_address={scenario}:{}:{}", result.host, result.port);
        }
    }

    #[test]
    fn redirects_default_port_even_when_the_original_host_has_no_addresses() {
        let result = resolve_with(
            "play.example.org",
            25565,
            |name| {
                assert_eq!(name, "_minecraft._tcp.play.example.org.");
                Some(("backend.example.org.".into(), 25570))
            },
            |host, port| {
                assert_eq!((host, port), ("backend.example.org.", 25570));
                Ok(vec!["127.0.0.1:25570".parse().unwrap()])
            },
        )
        .unwrap();
        assert_eq!(
            result.addresses,
            vec!["127.0.0.1:25570".parse::<SocketAddr>().unwrap()]
        );
    }

    #[test]
    fn no_record_or_lookup_failure_keeps_the_original_address() {
        resolve_with(
            "play.example.org",
            25565,
            |_| None,
            |host, port| {
                assert_eq!((host, port), ("play.example.org", 25565));
                Ok(vec![])
            },
        )
        .unwrap();
    }

    #[test]
    fn explicit_nondefault_ports_and_ip_literals_do_not_query_srv() {
        for (host, port) in [
            ("play.example.org", 25570),
            ("127.0.0.1", 25565),
            ("::1", 25565),
            ("localhost", 25565),
        ] {
            resolve_with(
                host,
                port,
                |_| panic!("unexpected SRV query"),
                |actual, actual_port| {
                    assert_eq!((actual, actual_port), (host, port));
                    Ok(vec![])
                },
            )
            .unwrap();
        }
    }

    #[test]
    fn fully_qualified_host_does_not_get_a_second_trailing_dot() {
        resolve_with(
            "play.example.org.",
            25565,
            |name| {
                assert_eq!(name, "_minecraft._tcp.play.example.org.");
                None
            },
            |_, _| Ok(vec![]),
        )
        .unwrap();
    }

    #[test]
    fn unknown_redirect_target_does_not_fall_back_to_the_original() {
        let result = resolve_with(
            "play.example.org",
            25565,
            |_| Some(("missing.example.org.".into(), 25570)),
            |host, _| {
                assert_eq!(host, "missing.example.org.");
                Err(Error::new("unknown redirected host"))
            },
        );
        assert_eq!(result.unwrap_err().to_string(), "unknown redirected host");
    }

    #[test]
    #[ignore = "daily"]
    fn literal_address_can_connect_without_dns() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let connection = crate::connection::Connection::connect_server(
            "127.0.0.1",
            address.port(),
            std::time::Duration::from_secs(1),
        )
        .unwrap();
        let (accepted, _) = listener.accept().unwrap();
        drop((connection, accepted));
    }
}
