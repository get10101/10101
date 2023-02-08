#[inline]
pub fn hex_str(value: &[u8]) -> String {
    let mut res = String::with_capacity(64);
    for v in value {
        res += &format!("{:02x}", v);
    }
    res
}
