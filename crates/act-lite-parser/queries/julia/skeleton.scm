; Modules and baremodules
(module_definition name: (identifier) @name) @module

; Long-form functions and macros; the name is the callee as written
(function_definition
  (signature
    [(identifier) @name
     (call_expression . [(identifier) (field_expression) (operator)] @name)
     (typed_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))
     (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))
     (where_expression . (typed_expression . (call_expression . [(identifier) (field_expression) (operator)] @name)))
     (typed_expression . (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name)))
     (where_expression . (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name)))
     (where_expression . (where_expression . (typed_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))))
     (where_expression . (typed_expression . (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))))])) @function
(macro_definition
  (signature
    (call_expression . [(identifier) (field_expression) (operator)] @name))) @macro

; Short-form functions: a call, or a typed or where call, on the left side
(assignment
  .
  [(call_expression . [(identifier) (field_expression) (operator)] @name)
   (typed_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))
   (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))
   (where_expression . (typed_expression . (call_expression . [(identifier) (field_expression) (operator)] @name)))
   (typed_expression . (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name)))
   (where_expression . (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name)))
   (where_expression . (where_expression . (typed_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))))
   (where_expression . (typed_expression . (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))))]) @function

; Types: the name is the first identifier of the type head
(struct_definition
  (type_head
    [(identifier) @name
     (parametrized_type_expression . (identifier) @name)
     (binary_expression . [(identifier) @name (parametrized_type_expression . (identifier) @name)])])) @struct
(abstract_definition
  (type_head
    [(identifier) @name
     (parametrized_type_expression . (identifier) @name)
     (binary_expression . [(identifier) @name (parametrized_type_expression . (identifier) @name)])])) @interface
(primitive_definition
  (type_head
    [(identifier) @name
     (parametrized_type_expression . (identifier) @name)
     (binary_expression . [(identifier) @name (parametrized_type_expression . (identifier) @name)])])) @type_alias

(const_statement
  (assignment . [(identifier) @name (typed_expression . (identifier) @name)])) @constant

; using M, import M, import M: x; one entry per module
(using_statement [(identifier) (scoped_identifier)] @name) @import
(import_statement [(identifier) (scoped_identifier)] @name) @import
(using_statement (selected_import . [(identifier) (scoped_identifier)] @name)) @import
(import_statement (selected_import . [(identifier) (scoped_identifier)] @name)) @import

; include("f.jl")
(call_expression
  . (identifier) @_call
  . (argument_list . (string_literal) @name .)
  (#eq? @_call "include")) @include
