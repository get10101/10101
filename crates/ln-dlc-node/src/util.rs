use lightning::ln::msgs::SocketAddress;
use std::net::IpAddr;
use std::net::SocketAddr;

#[inline]
pub fn hex_str(value: &[u8]) -> String {
    use std::fmt::Write as _; // import without risk of name clashing
    let mut s = String::new();

    for v in value {
        let _ = write!(s, "0x{v:02x}");
    }
    s
}

pub fn build_socket_address(ip: IpAddr, port: u16) -> SocketAddress {
    match ip {
        IpAddr::V4(ip) => SocketAddress::TcpIpV4 {
            addr: ip.octets(),
            port,
        },
        IpAddr::V6(ip) => SocketAddress::TcpIpV6 {
            addr: ip.octets(),
            port,
        },
    }
}

pub fn into_socket_addresses(address: SocketAddr) -> Vec<SocketAddress> {
    vec![build_socket_address(address.ip(), address.port())]
}
