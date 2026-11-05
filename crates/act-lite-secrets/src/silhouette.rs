/// The placeholder token a sequestered line carries in place of its value.
pub const SEQUESTERED_VALUE: &str = "#act:sequestered";

/// Header `act-lite env import` prepends once to a file it sequesters. Teaches the
/// protocol to any reader (human or agent). A file may hold sequestered and
/// plaintext lines side by side; the header says so.
pub const SILHOUETTE_HEADER: &str = "\
# ── act101 sequestered secrets ──────────────────────────────
# Lines that read KEY=#act:sequestered hold no value here; the real
# value lives outside this repo in act's per-project store. Any other
# line is ordinary plaintext config and is left exactly as it was.
# Use a sequestered secret by running your command through act — the
# value is injected into the child process and scrubbed (best-effort)
# from its output:
#
#     act-lite env run -- <your command>
#
# To deliberately read ONE value into context (rare, visible):
#     act-lite env get NAME --reveal
#
# This header holds no secret. Sequestered lines are safe to commit.
#
";

/// True when `buffer` already begins with the header's first line.
pub fn has_header(buffer: &str) -> bool {
    let first = SILHOUETTE_HEADER.lines().next().unwrap_or_default();
    buffer
        .lines()
        .next()
        .is_some_and(|l| l.trim_end_matches('\r') == first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_detected_only_at_the_top() {
        assert!(has_header(SILHOUETTE_HEADER));
        assert!(has_header(&format!("{SILHOUETTE_HEADER}K=v\n")));
        assert!(!has_header("K=v\n# ── act101 sequestered secrets\n"));
        assert!(!has_header(""));
    }
}
