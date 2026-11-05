; Capture all symbol definitions

; Namespace declarations
(namespace_declaration
  name: (_) @name) @definition

(file_scoped_namespace_declaration
  name: (_) @name) @definition

; Class declarations
(class_declaration
  name: (identifier) @name) @definition

; Record declarations
(record_declaration
  name: (identifier) @name) @definition

; Struct declarations
(struct_declaration
  name: (identifier) @name) @definition

; Interface declarations
(interface_declaration
  name: (identifier) @name) @definition

; Enum declarations
(enum_declaration
  name: (identifier) @name) @definition

; Enum members
(enum_member_declaration
  (identifier) @name) @definition

; Delegate declarations
(delegate_declaration
  name: (identifier) @name) @definition

; Method declarations
(method_declaration
  name: (identifier) @name) @definition

; Constructor declarations
(constructor_declaration
  name: (identifier) @name) @definition

; Property declarations
(property_declaration
  name: (identifier) @name) @definition

; Field declarations
(field_declaration
  (variable_declaration
    (variable_declarator
      (identifier) @name))) @definition

; Event field declarations
(event_field_declaration
  (variable_declaration
    (variable_declarator
      (identifier) @name))) @definition

; Parameters
(parameter
  name: (identifier) @name) @definition

; Type parameters
(type_parameter
  (identifier) @name) @definition

; Local variable declarations (inside methods)
(local_declaration_statement
  (variable_declaration
    (variable_declarator
      (identifier) @name) @definition))
