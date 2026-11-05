
; Scopes
(source_file) @local.scope
(block) @local.scope
(for_statement) @local.scope
(if_statement) @local.scope
(type_switch_statement) @local.scope

; Definitions — value bindings
(function_declaration name: (identifier) @local.definition)
(var_spec name: (identifier) @local.definition)
(const_spec name: (identifier) @local.definition)
(short_var_declaration left: (expression_list (identifier) @local.definition))
(range_clause left: (expression_list (identifier) @local.definition))
(type_switch_statement alias: (expression_list (identifier) @local.definition))
(parameter_declaration name: (identifier) @local.definition)

; Definitions — package and type bindings
(package_clause (package_identifier) @local.definition)
(import_spec name: (package_identifier) @local.definition)
(type_spec name: (type_identifier) @local.definition)

; Definitions — member bindings (methods; member-class candidates stay
; Unprovable by the resolver's kind guard, but the definition is modelled)
(method_declaration name: (field_identifier) @local.definition)

; References — value, type and package names (the package name is a name
; occurrence the kind guard must see, so it classifies Foreign against a
; same-named function, not vanish from the model). field_identifier
; (selector tails) is deliberately NOT a reference: member binding needs
; type resolution the model does not have.
(identifier) @local.reference
(type_identifier) @local.reference
(package_identifier) @local.reference
