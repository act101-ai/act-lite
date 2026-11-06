; Package
(package
  (full_ident) @name) @package

; Imports
(import
  path: (string) @name) @import

; Messages, including nested ones
(message
  (message_name
    (identifier) @name)) @struct

; Proto2 groups declare a message
(group
  (message_name
    (identifier) @name)) @struct

; Enums, including nested ones
(enum
  (enum_name
    (identifier) @name)) @enum

; Services
(service
  (service_name
    (identifier) @name)) @interface

; RPCs
(rpc
  (rpc_name
    (identifier) @name)) @method
