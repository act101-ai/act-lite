; Package
(package
  (full_ident) @name) @definition.module

; Messages, including nested ones
(message
  (message_name
    (identifier) @name)) @definition.struct

; Proto2 groups declare a message and a field
(group
  (message_name
    (identifier) @name)) @definition.struct

(group
  (message_name
    (identifier) @name)) @definition.field

; Enums, including nested ones
(enum
  (enum_name
    (identifier) @name)) @definition.enum

(enum_field
  (identifier) @name) @definition.enum_member

; Services and RPCs
(service
  (service_name
    (identifier) @name)) @definition.interface

(rpc
  (rpc_name
    (identifier) @name)) @definition.method

; Fields of messages; fields of extend blocks belong to another message
(message
  (message_body
    [
      (field (identifier) @name)
      (map_field (identifier) @name)
      (oneof (identifier) @name)
    ] @definition.field))

(oneof
  (oneof_field
    (identifier) @name) @definition.field)
