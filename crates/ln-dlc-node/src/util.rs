use lightning::ln::msgs::NetAddress;
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

pub fn build_net_address(ip: IpAddr, port: u16) -> NetAddress {
    match ip {
        IpAddr::V4(ip) => NetAddress::IPv4 {
            addr: ip.octets(),
            port,
        },
        IpAddr::V6(ip) => NetAddress::IPv6 {
            addr: ip.octets(),
            port,
        },
    }
}

pub fn into_net_addresses(address: SocketAddr) -> Vec<NetAddress> {
    vec![build_net_address(address.ip(), address.port())]
}
