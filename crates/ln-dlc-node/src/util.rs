#[inline]
pub fn hex_str(value: &[u8]) -> String {
    use std::fmt::Write as _; // import without risk of name clashing
    let mut s = String::new();

    for v in value {
        let _ = write!(s, "0x{v:02x}");
    }
    s
}
