; Top-level mapping keys — plain scalar keys
(block_mapping_pair
  key: (flow_node
    (plain_scalar
      (string_scalar) @name)) @variable)

; Top-level mapping keys — double-quoted keys
(block_mapping_pair
  key: (flow_node
    (double_quote_scalar) @name) @variable)

; Top-level mapping keys — single-quoted keys
(block_mapping_pair
  key: (flow_node
    (single_quote_scalar) @name) @variable)

; Anchors as named modules
(anchor
  (anchor_name) @name) @module
