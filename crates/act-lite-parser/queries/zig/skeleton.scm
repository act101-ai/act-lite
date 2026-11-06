; Functions at file level
(source_file
  (function_declaration name: (identifier) @name) @function)

; Functions inside a struct, enum, union or opaque
(struct_declaration
  (function_declaration name: (identifier) @name) @method)
(enum_declaration
  (function_declaration name: (identifier) @name) @method)
(union_declaration
  (function_declaration name: (identifier) @name) @method)
(opaque_declaration
  (function_declaration name: (identifier) @name) @method)

; test "name" blocks; a test named by an identifier is a use
(test_declaration (string (string_content) @name)) @function

; Types and imports
(variable_declaration . (identifier) @name "=" (struct_declaration)) @struct
(variable_declaration . (identifier) @name "=" [(enum_declaration) (error_set_declaration)]) @enum
(variable_declaration . (identifier) @name "=" (union_declaration)) @union
(variable_declaration . (identifier) @name "=" (expression) @value (#match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @import

; Other constants and variables at file or container level: the
; initializer is the expression after the `=` token (any initializer kind of
; the grammar except a container declaration), or the declaration has no
; `=` outside a line comment (extern). An exact @import is an import.
(source_file
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
((source_file
  (variable_declaration
    "const" . (identifier) @name type: (_)) @constant)
  (#match? @constant "^(//[^\\n]*|[^=/]|/[^/])*$"))
(struct_declaration
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
((struct_declaration
  (variable_declaration
    "const" . (identifier) @name type: (_)) @constant)
  (#match? @constant "^(//[^\\n]*|[^=/]|/[^/])*$"))
(enum_declaration
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
((enum_declaration
  (variable_declaration
    "const" . (identifier) @name type: (_)) @constant)
  (#match? @constant "^(//[^\\n]*|[^=/]|/[^/])*$"))
(union_declaration
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
((union_declaration
  (variable_declaration
    "const" . (identifier) @name type: (_)) @constant)
  (#match? @constant "^(//[^\\n]*|[^=/]|/[^/])*$"))
(opaque_declaration
  (variable_declaration
    "const" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
((opaque_declaration
  (variable_declaration
    "const" . (identifier) @name type: (_)) @constant)
  (#match? @constant "^(//[^\\n]*|[^=/]|/[^/])*$"))
(source_file
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
((source_file
  (variable_declaration
    "var" . (identifier) @name type: (_)) @variable)
  (#match? @variable "^(//[^\\n]*|[^=/]|/[^/])*$"))
(struct_declaration
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
((struct_declaration
  (variable_declaration
    "var" . (identifier) @name type: (_)) @variable)
  (#match? @variable "^(//[^\\n]*|[^=/]|/[^/])*$"))
(enum_declaration
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
((enum_declaration
  (variable_declaration
    "var" . (identifier) @name type: (_)) @variable)
  (#match? @variable "^(//[^\\n]*|[^=/]|/[^/])*$"))
(union_declaration
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
((union_declaration
  (variable_declaration
    "var" . (identifier) @name type: (_)) @variable)
  (#match? @variable "^(//[^\\n]*|[^=/]|/[^/])*$"))
(opaque_declaration
  (variable_declaration
    "var" . (identifier) @name "=" [(anonymous_struct_initializer) (anyframe_type) (array_type) (asm_expression) (assignment_expression) (async_expression) (await_expression) (binary_expression) (block) (boolean) (break_expression) (builtin_function) (builtin_type) (call_expression) (catch_expression) (character) (comptime_expression) (continue_expression) (dereference_expression) (error_type) (error_union_type) (field_expression) (float) (for_expression) (function_signature) (identifier) (if_expression) (index_expression) (integer) (labeled_type_expression) (multiline_string) (nosuspend_expression) (null_coercion_expression) (nullable_type) (opaque_declaration) (parenthesized_expression) (pointer_type) (range_expression) (resume_expression) (return_expression) (slice_type) (string) (struct_initializer) (switch_expression) (try_expression) (unary_expression) (while_expression) "undefined" "null" "unreachable" "anyframe"] @value
    (#not-match? @value "^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"(\\\\.|[^\"\\\\])*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
((opaque_declaration
  (variable_declaration
    "var" . (identifier) @name type: (_)) @variable)
  (#match? @variable "^(//[^\\n]*|[^=/]|/[^/])*$"))
