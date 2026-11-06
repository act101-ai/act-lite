; Functions
(source_file
  (function_declaration name: (identifier) @name) @definition.function)
(struct_declaration
  (function_declaration name: (identifier) @name) @definition.method)
(enum_declaration
  (function_declaration name: (identifier) @name) @definition.method)
(union_declaration
  (function_declaration name: (identifier) @name) @definition.method)
(opaque_declaration
  (function_declaration name: (identifier) @name) @definition.method)
(test_declaration (string (string_content) @name)) @definition.function

; Types
(variable_declaration . (identifier) @name "=" (struct_declaration)) @definition.struct
(variable_declaration . (identifier) @name "=" [(enum_declaration) (union_declaration) (error_set_declaration)]) @definition.enum

; Fields, enum members and error-set members; an empty container has a
; missing zero-width member, which is not reported
(struct_declaration
  (container_field name: (identifier) @name (#match? @name ".")) @definition.field)
(union_declaration
  (container_field name: (identifier) @name (#match? @name ".")) @definition.field)
(enum_declaration
  (container_field name: (identifier) @name (#match? @name ".")) @definition.enum_member)
(error_set_declaration (identifier) @name) @definition.enum_member

; Other constants and variables at file or container level: the
; initializer is the expression after the `=` token (any initializer kind of
; the grammar except a container declaration), or the declaration has none
; (extern: its last child is the type or an alignment, address-space or
; link-section annotation). An exact @import is an import.
(source_file
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(source_file
  (variable_declaration
    "const" . (identifier) @name type: (_) .) @definition.constant)
(source_file
  (variable_declaration
    "const" . (identifier) @name (byte_alignment) .) @definition.constant)
(source_file
  (variable_declaration
    "const" . (identifier) @name (address_space) .) @definition.constant)
(source_file
  (variable_declaration
    "const" . (identifier) @name (link_section) .) @definition.constant)
(struct_declaration
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(struct_declaration
  (variable_declaration
    "const" . (identifier) @name type: (_) .) @definition.constant)
(struct_declaration
  (variable_declaration
    "const" . (identifier) @name (byte_alignment) .) @definition.constant)
(struct_declaration
  (variable_declaration
    "const" . (identifier) @name (address_space) .) @definition.constant)
(struct_declaration
  (variable_declaration
    "const" . (identifier) @name (link_section) .) @definition.constant)
(enum_declaration
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(enum_declaration
  (variable_declaration
    "const" . (identifier) @name type: (_) .) @definition.constant)
(enum_declaration
  (variable_declaration
    "const" . (identifier) @name (byte_alignment) .) @definition.constant)
(enum_declaration
  (variable_declaration
    "const" . (identifier) @name (address_space) .) @definition.constant)
(enum_declaration
  (variable_declaration
    "const" . (identifier) @name (link_section) .) @definition.constant)
(union_declaration
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(union_declaration
  (variable_declaration
    "const" . (identifier) @name type: (_) .) @definition.constant)
(union_declaration
  (variable_declaration
    "const" . (identifier) @name (byte_alignment) .) @definition.constant)
(union_declaration
  (variable_declaration
    "const" . (identifier) @name (address_space) .) @definition.constant)
(union_declaration
  (variable_declaration
    "const" . (identifier) @name (link_section) .) @definition.constant)
(opaque_declaration
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(opaque_declaration
  (variable_declaration
    "const" . (identifier) @name type: (_) .) @definition.constant)
(opaque_declaration
  (variable_declaration
    "const" . (identifier) @name (byte_alignment) .) @definition.constant)
(opaque_declaration
  (variable_declaration
    "const" . (identifier) @name (address_space) .) @definition.constant)
(opaque_declaration
  (variable_declaration
    "const" . (identifier) @name (link_section) .) @definition.constant)
(source_file
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(source_file
  (variable_declaration
    "var" . (identifier) @name type: (_) .) @definition.variable)
(source_file
  (variable_declaration
    "var" . (identifier) @name (byte_alignment) .) @definition.variable)
(source_file
  (variable_declaration
    "var" . (identifier) @name (address_space) .) @definition.variable)
(source_file
  (variable_declaration
    "var" . (identifier) @name (link_section) .) @definition.variable)
(struct_declaration
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(struct_declaration
  (variable_declaration
    "var" . (identifier) @name type: (_) .) @definition.variable)
(struct_declaration
  (variable_declaration
    "var" . (identifier) @name (byte_alignment) .) @definition.variable)
(struct_declaration
  (variable_declaration
    "var" . (identifier) @name (address_space) .) @definition.variable)
(struct_declaration
  (variable_declaration
    "var" . (identifier) @name (link_section) .) @definition.variable)
(enum_declaration
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(enum_declaration
  (variable_declaration
    "var" . (identifier) @name type: (_) .) @definition.variable)
(enum_declaration
  (variable_declaration
    "var" . (identifier) @name (byte_alignment) .) @definition.variable)
(enum_declaration
  (variable_declaration
    "var" . (identifier) @name (address_space) .) @definition.variable)
(enum_declaration
  (variable_declaration
    "var" . (identifier) @name (link_section) .) @definition.variable)
(union_declaration
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(union_declaration
  (variable_declaration
    "var" . (identifier) @name type: (_) .) @definition.variable)
(union_declaration
  (variable_declaration
    "var" . (identifier) @name (byte_alignment) .) @definition.variable)
(union_declaration
  (variable_declaration
    "var" . (identifier) @name (address_space) .) @definition.variable)
(union_declaration
  (variable_declaration
    "var" . (identifier) @name (link_section) .) @definition.variable)
(opaque_declaration
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression)] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(opaque_declaration
  (variable_declaration
    "var" . (identifier) @name type: (_) .) @definition.variable)
(opaque_declaration
  (variable_declaration
    "var" . (identifier) @name (byte_alignment) .) @definition.variable)
(opaque_declaration
  (variable_declaration
    "var" . (identifier) @name (address_space) .) @definition.variable)
(opaque_declaration
  (variable_declaration
    "var" . (identifier) @name (link_section) .) @definition.variable)
