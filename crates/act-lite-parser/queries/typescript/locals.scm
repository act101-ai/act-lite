
; Scopes
(program) @local.scope
(statement_block) @local.scope
(for_statement) @local.scope
(for_in_statement) @local.scope
(arrow_function) @local.scope
(function_expression) @local.scope

; Definitions — value bindings. Plain parameters bind the `pattern` field
; in tree-sitter-typescript 0.23.2 (the crate's own locals.scm captures the
; direct identifier child for the same reason).
(variable_declarator name: (identifier) @local.definition)
(required_parameter pattern: (identifier) @local.definition)
(optional_parameter pattern: (identifier) @local.definition)
(assignment_pattern left: (identifier) @local.definition)
(pair_pattern value: (identifier) @local.definition)
(array_pattern (identifier) @local.definition)
(shorthand_property_identifier_pattern) @local.definition
(function_declaration name: (identifier) @local.definition)
(function_expression name: (identifier) @local.definition)
(for_in_statement left: (identifier) @local.definition)
(catch_clause parameter: (identifier) @local.definition)
(import_specifier name: (identifier) @local.definition)
(import_specifier alias: (identifier) @local.definition)
(enum_declaration name: (identifier) @local.definition)
(internal_module name: (identifier) @local.definition)

; Definitions — type bindings
(class_declaration name: (type_identifier) @local.definition)
(abstract_class_declaration name: (type_identifier) @local.definition)
(interface_declaration name: (type_identifier) @local.definition)
(type_alias_declaration name: (type_identifier) @local.definition)
(type_parameter name: (type_identifier) @local.definition)

; References — value and type names. property_identifier (member access
; tails, method names) is deliberately NOT a reference: member binding
; needs type resolution the model does not have.
(identifier) @local.reference
(type_identifier) @local.reference
