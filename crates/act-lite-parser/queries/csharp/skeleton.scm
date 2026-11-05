; Capture top-level declarations for file skeleton

; Namespace declarations
(namespace_declaration
  name: (_) @name) @namespace

(file_scoped_namespace_declaration
  name: (_) @name) @namespace

; Using directives
(using_directive) @import

; Class declarations
(class_declaration
  name: (identifier) @name) @class

; Record declarations (includes record struct in C# 10+)
(record_declaration
  name: (identifier) @name) @class

; Struct declarations
(struct_declaration
  name: (identifier) @name) @struct

; Interface declarations
(interface_declaration
  name: (identifier) @name) @interface

; Enum declarations
(enum_declaration
  name: (identifier) @name) @enum

; Delegate declarations
(delegate_declaration
  name: (identifier) @name) @delegate

; Field declarations
(field_declaration
  (variable_declaration
    (variable_declarator
      (identifier) @name))) @field

; Property declarations
(property_declaration
  name: (identifier) @name) @property

; Method declarations
(method_declaration
  name: (identifier) @name) @method

; Constructor declarations
(constructor_declaration
  name: (identifier) @name) @constructor

; Event field declarations
(event_field_declaration
  (variable_declaration
    (variable_declarator
      (identifier) @name))) @event

(indexer_declaration
  ("this") @name) @indexer

; Operator declarations (public static T operator +(T a, T b))
(operator_declaration
  operator: _ @name) @method

; Conversion operator declarations (implicit/explicit operator T(Foo f))
(conversion_operator_declaration
  type: (_) @name) @method
