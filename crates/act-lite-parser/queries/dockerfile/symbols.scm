; FROM stage aliases
(from_instruction
  as: (image_alias) @name) @definition

; ARG declarations
(arg_instruction
  name: (unquoted_string) @name) @definition

; ENV declarations
(env_instruction
  (env_pair
    name: (unquoted_string) @name)) @definition
