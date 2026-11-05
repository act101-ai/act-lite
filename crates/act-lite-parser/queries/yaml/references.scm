;; YAML references query — usages of named elements
;;
;; Captures:
;; - alias: references to anchors via aliases (*anchor_name)

(alias) @reference

;; Mapping keys: the declaring name (a value equal to a key is not one)
(block_mapping_pair
  key: (flow_node
    (plain_scalar
      (string_scalar) @reference)))

(anchor
  (anchor_name) @reference)
