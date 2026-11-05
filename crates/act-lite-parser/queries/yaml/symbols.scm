;; YAML symbols query — all named elements (keys)
;;
;; Captures:
;; - block_mapping_pair: captures key names in objects
;; - plain_scalar: the key identifier itself
;; - flow_mapping_pair: for flow-style objects
;; - anchor: named anchors (&name)

(block_mapping_pair
  (flow_node
    (plain_scalar
      (string_scalar) @name
    )
  ) @definition
)

(block_mapping_pair
  (flow_node
    (double_quote_scalar) @name
  ) @definition
)

(block_mapping_pair
  (flow_node
    (single_quote_scalar) @name
  ) @definition
)

(anchor
  (anchor_name) @name) @definition

;; Alias references
(alias) @reference
