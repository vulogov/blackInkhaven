/// Encode `data` as a lowercase hexadecimal string.
///
/// Each byte is emitted as exactly two hex digits. An empty slice produces an
/// empty string. The result is safe to embed in a DuckDB `from_hex(...)` call.
pub fn to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}
