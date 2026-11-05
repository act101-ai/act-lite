; Top-level function declarations (Dart grammar uses lambda_expression at top level)
(lambda_expression
  (function_signature
    (identifier) @name)) @function

; Class declarations
(class_definition
  (identifier) @name) @class

; Mixin declarations
(mixin_declaration
  (identifier) @name) @class

; Extension declarations
(extension_declaration
  (identifier) @name) @class

; Enum declarations
(enum_declaration
  (identifier) @name) @enum

; Import and export directives
(import_or_export) @import

(type_alias
  (type_identifier) @name
  .
  [(comment) (documentation_comment)]*
  .
  "=") @type_alias

(type_alias
  (type_identifier) @name
  .
  [(comment) (documentation_comment)]*
  .
  (type_parameters)
  .
  [(comment) (documentation_comment)]*
  .
  "=") @type_alias

(type_alias
  (type_identifier) @name
  .
  [(comment) (documentation_comment)]*
  .
  (formal_parameter_list)) @type_alias

(type_alias
  (type_identifier) @name
  .
  [(comment) (documentation_comment)]*
  .
  (type_parameters)
  .
  [(comment) (documentation_comment)]*
  .
  (formal_parameter_list)) @type_alias

; Instance methods, static methods, getters, setters
(class_member_definition
  (method_signature
    (function_signature
      (identifier) @name))) @method

; Named constructors: ClassName.factoryName(...)
(class_member_definition
  (method_signature
    (constructor_signature
      (identifier)
      "."
      (identifier) @name))) @constructor

; Primary constructors: ClassName(...)
(class_member_definition
  (declaration
    (constructor_signature
      (identifier) @name))) @constructor

; Class fields
(class_member_definition
  (declaration
    (initialized_identifier_list
      (initialized_identifier
        (identifier) @name)))) @field
