; Capture identifier references in expression contexts, plus the declaring
; names of types and methods (listed at the end).

; Method invocations where function is a simple identifier: foo()
(invocation_expression
  function: (identifier) @reference)

; Member accesses: obj.Property, obj.Field
(member_access_expression
  name: (identifier) @reference)

; Object creation (`new Foo()`) is covered by the type-position patterns below.

; Simple identifier expressions in assignments
(assignment_expression
  left: (identifier) @reference)

(assignment_expression
  right: (identifier) @reference)

; Argument usage: Foo(bar, baz)
(argument
  (identifier) @reference)

; Qualified names in using directives (import references)
(using_directive
  (qualified_name) @reference)

; Generic type arguments: List<Foo>, Dictionary<K, V>
(generic_name
  (type_argument_list
    (identifier) @reference))

(_
  type: [
    (identifier) @reference
    (qualified_name
      name: (identifier) @reference)
    (generic_name
      (identifier) @reference)
  ])

(_
  returns: [
    (identifier) @reference
    (qualified_name
      name: (identifier) @reference)
    (generic_name
      (identifier) @reference)
  ])

(base_list
  [
    (identifier) @reference
    (qualified_name
      name: (identifier) @reference)
    (generic_name
      (identifier) @reference)
  ])

(as_expression
  right: [
    (identifier) @reference
    (qualified_name
      name: (identifier) @reference)
    (generic_name
      (identifier) @reference)
  ])

(attribute
  name: [
    (identifier) @reference
    (qualified_name
      name: (identifier) @reference)
  ])

; Declaring names of types and methods
(class_declaration
  name: (identifier) @reference)

(struct_declaration
  name: (identifier) @reference)

(interface_declaration
  name: (identifier) @reference)

(enum_declaration
  name: (identifier) @reference)

(record_declaration
  name: (identifier) @reference)

(method_declaration
  name: (identifier) @reference)

(constructor_declaration
  name: (identifier) @reference)

(delegate_declaration
  name: (identifier) @reference)
