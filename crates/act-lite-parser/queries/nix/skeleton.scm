; Bindings in attribute sets and let blocks, kinded by the bound expression
(binding
  attrpath: (attrpath) @name
  expression: (function_expression)) @function

(binding
  attrpath: (attrpath) @name
  expression: [(attrset_expression) (rec_attrset_expression)]) @module

(binding
  attrpath: (attrpath) @name
  expression: [
    (apply_expression) (assert_expression) (binary_expression)
    (float_expression) (has_attr_expression) (hpath_expression)
    (if_expression) (indented_string_expression) (integer_expression)
    (let_attrset_expression) (let_expression) (list_expression)
    (parenthesized_expression) (path_expression) (select_expression)
    (spath_expression) (string_expression) (unary_expression)
    (uri_expression) (variable_expression) (with_expression)
  ]) @variable

; inherit a b; and inherit (s) a;
(inherit
  attrs: (inherited_attrs
    attr: [(identifier) (string_expression)] @name)) @variable

(inherit_from
  attrs: (inherited_attrs
    attr: [(identifier) (string_expression)] @name)) @variable

; import <path>: the application of import to the path only
(apply_expression
  function: (variable_expression
    name: (identifier) @_import
    (#eq? @_import "import"))
  argument: (_) @name) @import
