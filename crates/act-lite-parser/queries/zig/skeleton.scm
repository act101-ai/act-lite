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
(variable_declaration . (identifier) @name . (struct_declaration)) @struct
(variable_declaration . (identifier) @name . [(enum_declaration) (error_set_declaration)]) @enum
(variable_declaration . (identifier) @name . (union_declaration)) @union
(variable_declaration . (identifier) @name (builtin_function (builtin_identifier) @callee (arguments (string)) (#eq? @callee "@import")) . ";") @import

; Other constants and variables at file or container level
(source_file
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
(struct_declaration
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
(enum_declaration
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
(union_declaration
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
(opaque_declaration
  (variable_declaration
    "const" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @constant)
(source_file
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
(struct_declaration
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
(enum_declaration
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
(union_declaration
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
(opaque_declaration
  (variable_declaration
    "var" . (identifier) @name (_) @value . ";"
    (#not-match? @value "(?s)^((packed|extern)\\s+)?(struct|enum|union|error)\\b.*\\}$|^@import(\\s|//[^\\n]*)*\\((\\s|//[^\\n]*)*\"[^\"]*\"(\\s|//[^\\n]*|,)*\\)$")) @variable)
