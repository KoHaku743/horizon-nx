//! The console's DNS servers, for the SRV record a server address is looked up by first (the
//! Switch has no resolv.conf): nifm's, the ones the console's internet settings give its
//! connection (libnx `nifmGetCurrentIpConfigInfo`, addresses in network order).
//!
//! nifm is asked once, on a thread of its own started with the game, and joining a server only
//! reads what it answered. The first build asked nifm on the network thread at every join and
//! froze the console as a server was joined (#3); each step is logged so a hang names its place,
//! and nothing waits for nifm: until it answers, servers are joined without an SRV lookup.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::OnceLock;

unsafe extern "C" {
    fn nifmInitialize(service_type: u32) -> u32;
    fn nifmExit();
    fn nifmGetCurrentIpConfigInfo(
        current_addr: *mut u32,
        subnet_mask: *mut u32,
        gateway: *mut u32,
        primary_dns_server: *mut u32,
        secondary_dns_server: *mut u32,
    ) -> u32;
}

/// libnx `NifmServiceType_User`.
const USER: u32 = 0;

static SERVERS: OnceLock<Vec<SocketAddr>> = OnceLock::new();

/// Starts reading the connection's DNS servers, unless `HORIZON_NX_SRV=0` (env.txt) turns SRV
/// lookups off. Returns whether they are on: the caller then registers [`name_servers`].
pub fn start() -> bool {
    if std::env::var_os("HORIZON_NX_SRV").is_some_and(|v| v == "0") {
        crate::logging::write_log_line(format_args!(
            "[DNS] SRV lookups off (HORIZON_NX_SRV=0): servers are joined at the address as typed"
        ));
        return false;
    }
    let spawned = std::thread::Builder::new()
        .name("DNS servers".into())
        .stack_size(64 * 1024)
        .spawn(|| {
            crate::logging::write_log_line(format_args!(
                "[DNS] asking nifm for the connection's DNS servers"
            ));
            let servers = read();
            crate::logging::write_log_line(format_args!(
                "[DNS] DNS servers: {servers:?}{}",
                if servers.is_empty() {
                    " (none: servers are joined without an SRV lookup)"
                } else {
                    ""
                }
            ));
            let _ = SERVERS.set(servers);
        });
    if let Err(e) = spawned {
        crate::logging::write_log_line(format_args!(
            "[DNS] could not start the DNS servers' thread: {e}; no SRV lookups"
        ));
        return false;
    }
    true
}

/// What nifm answered at startup: the primary and secondary DNS servers, port 53. Empty until it
/// answered, or without a connection.
pub fn name_servers() -> Vec<SocketAddr> {
    match SERVERS.get() {
        Some(servers) => servers.clone(),
        None => {
            crate::logging::write_log_line(format_args!(
                "[DNS] nifm hasn't answered yet: joining without an SRV lookup"
            ));
            Vec::new()
        }
    }
}

fn read() -> Vec<SocketAddr> {
    let (mut addr, mut mask, mut gateway, mut primary, mut secondary) = (0, 0, 0, 0, 0);
    // SAFETY: nifm is reference-counted by libnx: this opens and closes a use of it; the query
    // writes five u32s.
    let rc = unsafe {
        match nifmInitialize(USER) {
            0 => {
                crate::logging::write_log_line(format_args!("[DNS] nifm open"));
                let rc = nifmGetCurrentIpConfigInfo(
                    &mut addr,
                    &mut mask,
                    &mut gateway,
                    &mut primary,
                    &mut secondary,
                );
                nifmExit();
                rc
            }
            rc => rc,
        }
    };
    if rc != 0 {
        crate::logging::write_log_line(format_args!(
            "[DNS] no DNS servers from the connection (0x{rc:x})"
        ));
        return Vec::new();
    }
    let mut servers = Vec::new();
    for raw in [primary, secondary] {
        let ip = Ipv4Addr::from(raw.to_ne_bytes());
        let server = SocketAddr::new(ip.into(), 53);
        if !ip.is_unspecified() && !servers.contains(&server) {
            servers.push(server);
        }
    }
    servers
}
