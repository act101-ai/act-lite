
; Scopes
(program) @local.scope
(statement_block) @local.scope
(for_statement) @local.scope
(for_in_statement) @local.scope
(arrow_function) @local.scope
(function_expression) @local.scope

; Definitions — value bindings
(variable_declarator name: (identifier) @local.definition)
(formal_parameters (identifier) @local.definition)
(assignment_pattern left: (identifier) @local.definition)
(pair_pattern value: (identifier) @local.definition)
(array_pattern (identifier) @local.definition)
(shorthand_property_identifier_pattern) @local.definition
(function_declaration name: (identifier) @local.definition)
(function_expression name: (identifier) @local.definition)
; A class declaration binds its name as a VALUE (the constructor) — the
; `new Logger()` constructor reference (A-fix round 2, javascript/move-003).
; JavaScript names a class with a plain identifier (TS: type_identifier).
(class_declaration name: (identifier) @local.definition)
(for_in_statement left: (identifier) @local.definition)
(catch_clause parameter: (identifier) @local.definition)
(import_specifier name: (identifier) @local.definition)
(import_specifier alias: (identifier) @local.definition)

; References — value names; property_identifier tails stay Unprovable
; (see the TypeScript model's note).
(identifier) @local.reference
