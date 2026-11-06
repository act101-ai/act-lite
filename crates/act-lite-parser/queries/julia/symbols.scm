; Modules and baremodules
(module_definition name: (identifier) @name) @definition.module

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
     (where_expression . (typed_expression . (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))))])) @definition.function
(macro_definition
  (signature
    (call_expression . [(identifier) (field_expression) (operator)] @name))) @definition.function

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
   (where_expression . (typed_expression . (where_expression . (call_expression . [(identifier) (field_expression) (operator)] @name))))]) @definition.function

; Types: the name is the first identifier of the type head
(struct_definition
  (type_head
    [(identifier) @name
     (parametrized_type_expression . (identifier) @name)
     (binary_expression . [(identifier) @name (parametrized_type_expression . (identifier) @name)])])) @definition.struct
(abstract_definition
  (type_head
    [(identifier) @name
     (parametrized_type_expression . (identifier) @name)
     (binary_expression . [(identifier) @name (parametrized_type_expression . (identifier) @name)])])) @definition.interface
(primitive_definition
  (type_head
    [(identifier) @name
     (parametrized_type_expression . (identifier) @name)
     (binary_expression . [(identifier) @name (parametrized_type_expression . (identifier) @name)])])) @definition.type_alias

(const_statement
  (assignment . [(identifier) @name (typed_expression . (identifier) @name)])) @definition.constant

; Struct fields
(struct_definition [(identifier) @name (typed_expression . (identifier) @name)]) @definition.field
