
; Scopes
(translation_unit) @local.scope
(compound_statement) @local.scope

; Definitions — value bindings
(declaration declarator: (identifier) @local.definition)
(init_declarator declarator: (identifier) @local.definition)
(pointer_declarator declarator: (identifier) @local.definition)
(function_declarator declarator: (identifier) @local.definition)
(parameter_declaration declarator: (identifier) @local.definition)
(parameter_declaration declarator: (pointer_declarator declarator: (identifier) @local.definition))
(enumerator name: (identifier) @local.definition)
(preproc_def name: (identifier) @local.definition)
(preproc_function_def name: (identifier) @local.definition)

; Definitions — type bindings
(type_definition declarator: (type_identifier) @local.definition)
(struct_specifier name: (type_identifier) @local.definition body: (field_declaration_list))
(union_specifier name: (type_identifier) @local.definition body: (field_declaration_list))
(enum_specifier name: (type_identifier) @local.definition body: (enumerator_list))

; References — value and type names. field_identifier (member access) is
; deliberately NOT a reference: member binding needs type resolution the
; model does not have, so member tails stay Unprovable (refuse).
(identifier) @local.reference
(type_identifier) @local.reference
