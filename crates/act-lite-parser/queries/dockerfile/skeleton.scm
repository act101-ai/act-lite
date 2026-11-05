; FROM instruction — the stage alias is the declared name
(from_instruction
  as: (image_alias) @name) @module

; ARG — build argument
(arg_instruction
  name: (unquoted_string) @name) @variable

; ENV — environment variable
(env_instruction
  (env_pair
    name: (unquoted_string) @name)) @variable
