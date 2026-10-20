(identifier) @variable

(comment) @comment

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ":" "."] @punctuation.delimiter

[
  "state"
  "let"
  "fn"
  "view"
  "struct"
] @keyword

[
  "if"
  "else"
  "for"
  "in"
  "return"
] @keyword

(boolean) @boolean
(integer) @number
(float) @number

(string) @string
(escape_sequence) @string.escape

(type_identifier) @type
((type_identifier) @type.builtin
  (#any-of? @type.builtin "int" "float" "bool" "string"))

(struct_declaration name: (identifier) @type)
(view_declaration name: (identifier) @type)
(function_declaration name: (identifier) @function)

(parameter name: (identifier) @variable.parameter)
(field_declaration name: (identifier) @property)
(named_argument label: (identifier) @label)
(state_declaration name: (identifier) @variable.special)
(for_statement index: (identifier) @variable)
(for_statement item: (identifier) @variable)
(field_expression field: (identifier) @property)
(implicit_member_expression "." @punctuation.delimiter member: (identifier) @variant)

(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (identifier) @function.method))
((call_expression function: (identifier) @type)
  (#match? @type "^[A-Z]"))
((call_expression function: (identifier) @function.builtin)
  (#any-of? @function.builtin
    "window" "column" "row" "label" "button" "text_field" "checkbox" "nav_item" "spacer"
    "divider"
    "download" "pick_file" "pick_files" "pick_folder" "pick_save_path"))
((call_expression function: (field_expression field: (identifier) @function.method.builtin))
  (#any-of? @function.method.builtin "len" "is_empty" "push" "remove"))

[
  "="
  "+="
  "-="
  "*="
  "/="
  "+"
  "-"
  "*"
  "/"
  "%"
  "!"
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "&&"
  "||"
  "->"
  ".."
] @operator

(unary_expression operator: "$" @operator.special)

(interpolation
  "{" @punctuation.special
  "}" @punctuation.special)
