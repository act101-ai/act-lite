; function(name ...) ... endfunction()
(function_def
  (function_command
    (argument_list
      .
      (argument
        [
          (unquoted_argument) @name
          (quoted_argument
            (quoted_element) @name)
          (bracket_argument
            (bracket_argument_content) @name)
        ])))) @definition.function

; macro(name ...) ... endmacro()
(macro_def
  (macro_command
    (argument_list
      .
      (argument
        [
          (unquoted_argument) @name
          (quoted_argument
            (quoted_element) @name)
          (bracket_argument
            (bracket_argument_content) @name)
        ])))) @definition.function

; project(name ...)
(normal_command
  (identifier) @command
  (argument_list
    .
    (argument
      [
        (unquoted_argument) @name
        (quoted_argument
          (quoted_element) @name)
        (bracket_argument
          (bracket_argument_content) @name)
      ]))
  (#match? @command "^(?i:project)$")) @definition.module

; Build targets
(normal_command
  (identifier) @command
  (argument_list
    .
    (argument
      [
        (unquoted_argument) @name
        (quoted_argument
          (quoted_element) @name)
        (bracket_argument
          (bracket_argument_content) @name)
      ]))
  (#match? @command "^(?i:add_executable|add_library|add_custom_target)$")) @definition.class

; Variables and options
(normal_command
  (identifier) @command
  (argument_list
    .
    (argument
      [
        (unquoted_argument) @name
        (quoted_argument
          (quoted_element) @name)
        (bracket_argument
          (bracket_argument_content) @name)
      ]))
  (#match? @command "^(?i:set|option)$")) @definition.variable
