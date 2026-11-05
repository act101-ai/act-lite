
; Scopes
(source_file) @local.scope
(block) @local.scope
; A module's items bind inside it: a bare `helper()` outside `mod m` must
; not prove against `m::helper` (it is a different binding in real Rust).
(mod_item body: (declaration_list) @local.scope)

; Definitions — value bindings
(let_declaration pattern: (identifier) @local.definition)
(parameter pattern: (identifier) @local.definition)
(closure_parameters (identifier) @local.definition)
(for_expression pattern: (identifier) @local.definition)
(match_pattern (identifier) @local.definition)
(tuple_pattern (identifier) @local.definition)
(tuple_struct_pattern (identifier) @local.definition)
(function_item name: (identifier) @local.definition)
(const_item name: (identifier) @local.definition)
(static_item name: (identifier) @local.definition)
(enum_variant name: (identifier) @local.definition)
(mod_item name: (identifier) @local.definition)
(macro_definition name: (identifier) @local.definition)
(use_as_clause alias: (identifier) @local.definition)

; Definitions — type bindings
(struct_item name: (type_identifier) @local.definition)
(union_item name: (type_identifier) @local.definition)
(enum_item name: (type_identifier) @local.definition)
(trait_item name: (type_identifier) @local.definition)
(type_item name: (type_identifier) @local.definition)

; References — value and type names. field_identifier (member access) and
; scoped path segments are handled by the resolver's context guards.
(identifier) @local.reference
(type_identifier) @local.reference
