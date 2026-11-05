/// Values shorter than this are not redacted, so common short strings like
/// `true` or `8080` don't scrub unrelated output.
pub const MIN_REDACT_LEN: usize = 6;
