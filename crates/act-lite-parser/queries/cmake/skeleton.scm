; function(name ...) ... endfunction()
(function_def
  (function_command
    (argument_list
      .
      (argument) @name))) @function

; macro(name ...) ... endmacro()
(macro_def
  (macro_command
    (argument_list
      .
      (argument) @name))) @macro

; project(name ...)
(normal_command
  (identifier) @command
  (argument_list
    .
    (argument) @name)
  (#match? @command "^(?i:project)$")) @module

; Build targets
(normal_command
  (identifier) @command
  (argument_list
    .
    (argument) @name)
  (#match? @command "^(?i:add_executable|add_library|add_custom_target)$")) @class

; Variables and options
(normal_command
  (identifier) @command
  (argument_list
    .
    (argument) @name)
  (#match? @command "^(?i:set|option)$")) @variable

; Included files and subdirectories
(normal_command
  (identifier) @command
  (argument_list
    .
    (argument) @name)
  (#match? @command "^(?i:include|add_subdirectory)$")) @include

; Packages
(normal_command
  (identifier) @command
  (argument_list
    .
    (argument) @name)
  (#match? @command "^(?i:find_package)$")) @import
