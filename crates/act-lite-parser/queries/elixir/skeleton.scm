; Module definitions
(call
  target: (identifier) @defmodule (#eq? @defmodule "defmodule")
  (arguments (alias) @name)) @module

; Public function definitions
(call
  target: (identifier) @def (#eq? @def "def")
  (arguments
    (call
      target: (identifier) @name))) @function

; Private function definitions
(call
  target: (identifier) @defp (#eq? @defp "defp")
  (arguments
    (call
      target: (identifier) @name))) @function

; Macro definitions
(call
  target: (identifier) @defmacro (#match? @defmacro "^defmacro")
  (arguments
    (call
      target: (identifier) @name))) @function

; Protocol definitions
(call
  target: (identifier) @defprotocol (#eq? @defprotocol "defprotocol")
  (arguments (alias) @name)) @interface

; Protocol implementations
(call
  target: (identifier) @defimpl (#eq? @defimpl "defimpl")
  (arguments (alias) @name)) @class

(call
  target: (identifier) @name (#eq? @name "defstruct")) @class

; Type specifications
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @type_attr (#match? @type_attr "^(type|typep|opaque)$")
    (arguments
      (binary_operator
        left: [
          (identifier) @name
          (call target: (identifier) @name)
        ])))) @type_alias

(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @name
    (#not-match? @name "^(doc|moduledoc|spec|type|typep|opaque|callback|impl|behaviour|behavior)$"))) @constant

; Import directives
(call
  target: (identifier) @directive (#match? @directive "^(alias|import|use|require)$")) @import

; Guard and delegate definitions (defguard, defguardp, defdelegate)
(call
  target: (identifier) @_guard_delegate
  (arguments
    [
      (call target: (identifier) @name)
      (binary_operator
        left: (call target: (identifier) @name)
        operator: "when")
    ])
  (#any-of? @_guard_delegate "defguard" "defguardp" "defdelegate")) @function
