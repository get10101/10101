use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use dlc_manager::ReferenceId;
use lightning::ln::msgs::SocketAddress;
use std::net::IpAddr;
use std::net::SocketAddr;
use uuid::Uuid;

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

pub fn parse_from_uuid(uuid: Uuid) -> ReferenceId {
    let hex = uuid.as_simple().to_hex();
    let bytes = hex.as_bytes();

    debug_assert!(bytes.len() == 32, "length must be exactly 32 bytes");

    let mut array = [0u8; 32];
    array.copy_from_slice(bytes);

    array
}

pub fn parse_from_reference_id(reference_id: ReferenceId) -> Result<Uuid> {
    let reference_id = hex::decode(reference_id)?;
    let reference_id = reference_id.to_hex();
    let reference_id = Uuid::parse_str(&reference_id)?;

    Ok(reference_id)
}

/// Returns the reference id as printed uuid. If None or invalid the default uuid to string will be
/// returned. default value: 00000000-0000-0000-0000-000000000000
pub fn stringify_reference_id(reference_id: Option<ReferenceId>) -> String {
    match reference_id {
        Some(reference_id) => parse_from_reference_id(reference_id).unwrap_or(
            Uuid::parse_str("00000000-0000-0000-0000-000000000000").expect("valid dummy uuid"),
        ),
        None => Uuid::parse_str("00000000-0000-0000-0000-000000000000").expect("valid dummy uuid"),
    }
    .to_string()
}

#[cfg(test)]
mod test {
    use crate::util::parse_from_reference_id;
    use crate::util::parse_from_uuid;
    use uuid::Uuid;

    #[test]
    fn convert_uuid_to_reference_id() {
        let id = Uuid::new_v4();

        let reference_id = parse_from_uuid(id);
        let parsed_id = parse_from_reference_id(reference_id).unwrap();

        assert_eq!(id, parsed_id);
    }
}
