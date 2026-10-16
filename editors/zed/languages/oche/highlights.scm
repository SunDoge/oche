; The initial Oche grammar is pinned to tree-sitter-rust because Oche 0.1 keeps
; Rust-shaped declarations, patterns, expressions, and postfix await. These
; queries add Oche-only words and deliberately use only nodes from that grammar.

(line_comment) @comment
(block_comment) @comment

(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(escape_sequence) @string.escape

(integer_literal) @number
(float_literal) @number
(boolean_literal) @boolean

(type_identifier) @type
(primitive_type) @type.builtin
((type_identifier) @type.builtin
  (#any-of? @type.builtin "Int" "Float" "Bool" "String" "List" "Future" "Option" "Result"))

(field_identifier) @property
(parameter (identifier) @variable.parameter)

(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression
  function: (field_expression field: (field_identifier) @function.method))

((identifier) @constructor
  (#match? @constructor "^[A-Z]"))

[
  "as"
  "async"
  "await"
  "else"
  "enum"
  "fn"
  "if"
  "let"
  "match"
  "pub"
  "struct"
  "try"
] @keyword

((identifier) @keyword
  (#any-of? @keyword "comptime" "const" "dart" "import" "mut" "test"))

[
  "->"
  "=>"
  "="
  "+"
  "-"
  "*"
  "?"
] @operator

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[":" "," "." ";" "::"] @punctuation.delimiter
