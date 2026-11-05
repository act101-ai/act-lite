; TOML references query — key identifier usages
;
; In TOML there are no true cross-references (no aliases like YAML),
; but we capture all key identifiers as potential references for rename support.

(bare_key) @reference

(quoted_key) @reference
