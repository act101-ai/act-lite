(function_declaration
  name: (simple_identifier) @name) @definition

(deinit_declaration
  "deinit" @name) @definition

(init_declaration
  "init" @name) @definition

; Class/Struct/Enum/Actor declarations
(class_declaration
  name: (type_identifier) @name) @definition

(class_declaration
  name: (user_type
    (type_identifier) @name)) @definition

; Protocol declarations
(protocol_declaration
  name: (type_identifier) @name) @definition

; Property declarations (includes computed properties)
(property_declaration
  (pattern
    (simple_identifier) @name)) @definition

; Subscript declarations — named by their keyword, as for init/deinit above
(subscript_declaration
  "subscript" @name) @definition

; Type aliases
(typealias_declaration
  name: (type_identifier) @name) @definition

; Associated types
(associatedtype_declaration
  name: (type_identifier) @name) @definition

; Function parameters
(parameter
  name: (simple_identifier) @name) @definition

; For loop variable
(for_statement
  (pattern
    (simple_identifier) @name)) @definition

; Generic type parameters
(type_parameter
  (type_identifier) @name) @definition

; Protocol function requirements
(protocol_function_declaration
  name: (simple_identifier) @name) @definition

; Protocol property requirements
(protocol_property_declaration
  (pattern
    (simple_identifier) @name)) @definition
