; All identifiers
(identifier) @reference

; Instance variables
(instance_variable) @reference

; Class variables
(class_variable) @reference

; Global variables
(global_variable) @reference

; Constants
(constant) @reference

; Scope resolution (Module::Class)
(scope_resolution
  name: (constant) @reference)

; Method calls
(call
  method: (identifier) @reference)

; Element reference (array/hash access)
(element_reference
  object: (identifier) @reference)

; Simple symbol references
(simple_symbol) @reference

; Superclass references
(superclass
  (constant) @reference)

; Include/extend/prepend module references
(call
  method: (identifier) @include_method (#match? @include_method "^(include|extend|prepend)$")
  arguments: (argument_list
    (constant) @reference))

; Block yield
(yield) @reference

; Self references
(self) @reference

; Attribute method references
(call
  method: (identifier) @attr_method (#match? @attr_method "^attr_(reader|writer|accessor)$")
  arguments: (argument_list
    (simple_symbol) @reference))

; Method definition call (define_method)
(call
  method: (identifier) @define_method (#eq? @define_method "define_method")
  arguments: (argument_list
    (simple_symbol) @reference))

; Class_eval/instance_eval/module_eval
(call
  method: (identifier) @eval_method (#match? @eval_method "^(class_eval|instance_eval|module_eval)$"))

; Require/require_relative references
(call
  method: (identifier) @require_method (#match? @require_method "^require")
  arguments: (argument_list
    (string) @reference))
