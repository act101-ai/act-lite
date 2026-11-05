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
(variable_declaration . (identifier) @name . (struct_declaration)) @definition.struct
(variable_declaration . (identifier) @name . [(enum_declaration) (union_declaration) (error_set_declaration)]) @definition.enum

; Fields, enum members and error-set members; an empty container has a
; missing zero-width member, which is not reported
(struct_declaration
  (container_field name: (identifier) @name (#match? @name ".")) @definition.field)
(union_declaration
  (container_field name: (identifier) @name (#match? @name ".")) @definition.field)
(enum_declaration
  (container_field name: (identifier) @name (#match? @name ".")) @definition.enum_member)
(error_set_declaration (identifier) @name) @definition.enum_member

; Other constants and variables at file or container level
(source_file
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(struct_declaration
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(enum_declaration
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(union_declaration
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(opaque_declaration
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.constant)
(source_file
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(struct_declaration
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(enum_declaration
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(union_declaration
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
(opaque_declaration
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @definition.variable)
