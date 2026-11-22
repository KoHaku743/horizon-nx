//! The console's DNS servers, for the SRV record a server address is looked up by first (the
//! Switch has no resolv.conf): nifm's, the ones the console's internet settings give its
//! connection (libnx `nifmGetCurrentIpConfigInfo`, addresses in network order).

use std::net::{Ipv4Addr, SocketAddr};

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

/// The connection's primary and secondary DNS servers, port 53; empty without a connection.
pub fn name_servers() -> Vec<SocketAddr> {
    let (mut addr, mut mask, mut gateway, mut primary, mut secondary) = (0, 0, 0, 0, 0);
    // SAFETY: nifm is reference-counted by libnx: this opens and closes a use of it; the query
    // writes five u32s.
    let rc = unsafe {
        match nifmInitialize(USER) {
            0 => {
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
        log::warn!("Horizon: no DNS servers from the connection (0x{rc:x})");
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
