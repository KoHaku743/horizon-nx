use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::os::fd::{AsRawFd, RawFd};

/// Waker backed by a UDP socket on the loopback address that sends to itself (Horizon patch).
///
/// The Nintendo Switch's newlib (libnx) has neither `pipe(2)` nor `eventfd(2)`, and its threads
/// are real, so the boolean `single_threaded` waker cannot interrupt a `poll(2)` another thread
/// is blocked in. A datagram to the waker's own port makes its descriptor readable, as a byte in
/// a pipe would.
#[derive(Debug)]
pub(crate) struct Waker {
    socket: UdpSocket,
}

impl Waker {
    pub(crate) fn new_unregistered() -> io::Result<Waker> {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
        socket.connect(socket.local_addr()?)?;
        socket.set_nonblocking(true)?;
        Ok(Waker { socket })
    }

    pub(crate) fn wake(&self) -> io::Result<()> {
        match self.socket.send(&[1]) {
            Ok(_) => Ok(()),
            // The receive buffer is full of earlier wake-ups: the poller is woken already.
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(ref err) if err.kind() == io::ErrorKind::Interrupted => self.wake(),
            Err(err) => Err(err),
        }
    }

    pub(crate) fn ack_and_reset(&self) {
        let mut buf = [0; 64];
        while matches!(self.socket.recv(&mut buf), Ok(n) if n > 0) {}
    }

    pub(crate) fn fd(&self) -> Option<RawFd> {
        Some(self.socket.as_raw_fd())
    }

    /// Only ever `true` for the `single_threaded.rs` implementation.
    pub(crate) fn woken(&self) -> bool {
        false
    }
}
