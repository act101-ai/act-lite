; Message, enum and rpc types, one identifier at a time
(message_or_enum_type
  (identifier) @reference)

; Declaration names
(message_name (identifier) @reference)
(enum_name (identifier) @reference)
(service_name (identifier) @reference)
(rpc_name (identifier) @reference)
(enum_field . (identifier) @reference)
(field . (type) . (identifier) @reference)
