; Module definitions
(call
  target: (identifier) @defmodule (#eq? @defmodule "defmodule")
  (arguments (alias) @name)) @definition

; Function definitions (def)
(call
  target: (identifier) @def (#eq? @def "def")
  (arguments
    (call
      target: (identifier) @name))) @definition

; Function and macro definitions with a guard (def f(x) when x > 0, def f when true, do: 1).
; The field order follows the tree: left, operator.
(call
  target: (identifier) @_def
  (arguments
    .
    (binary_operator
      left: [
        (call target: (identifier) @name)
        (identifier) @name
      ]
      operator: "when"))
  (#any-of? @_def "def" "defp" "defmacro" "defmacrop")) @definition

; Private function definitions (defp)
(call
  target: (identifier) @defp (#eq? @defp "defp")
  (arguments
    (call
      target: (identifier) @name))) @definition

; Macro definitions
(call
  target: (identifier) @defmacro (#match? @defmacro "^defmacrop?$")
  (arguments
    (call
      target: (identifier) @name))) @definition

; Function and macro definitions without parentheses (def name do, def name, do: x)
(call
  target: (identifier) @_def
  (arguments . (identifier) @name)
  (#any-of? @_def "def" "defp" "defmacro" "defmacrop")) @definition

; Protocol definitions
(call
  target: (identifier) @defprotocol (#eq? @defprotocol "defprotocol")
  (arguments (alias) @name)) @definition


; Match operator bindings
(binary_operator
  operator: "="
  left: (identifier) @name) @definition

; Module attributes
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @name)) @definition

; Capture operator
(unary_operator
  operator: "&"
  operand: (binary_operator
    operator: "/"
    left: (identifier) @name)) @definition

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
  (#any-of? @_guard_delegate "defguard" "defguardp" "defdelegate")) @definition
