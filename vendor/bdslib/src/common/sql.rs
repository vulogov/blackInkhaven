/// Escape a string for safe interpolation into a SQL single-quoted literal.
///
/// Doubles every `'` character (`'` → `''`), which is the standard SQL escape
/// for single quotes inside string literals.
pub fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}
