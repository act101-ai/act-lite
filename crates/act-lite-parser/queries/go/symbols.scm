; Package
(package_clause
  (package_identifier) @name) @definition

; Functions
(function_declaration
  name: (identifier) @name) @definition

; Methods
(method_declaration
  name: (field_identifier) @name) @definition

; Type declarations
(type_spec
  name: (type_identifier) @name) @definition

; Struct fields
(field_declaration
  name: (field_identifier) @name) @definition

; Const specs
(const_spec
  name: (identifier) @name) @definition

; Var specs
(var_spec
  name: (identifier) @name) @definition

; Short variable declarations
(short_var_declaration
  left: (expression_list
    (identifier) @name)) @definition

; Parameters
(parameter_declaration
  name: (identifier) @name) @definition

; Type parameters (generics)
(type_parameter_declaration
  (identifier) @name) @definition
