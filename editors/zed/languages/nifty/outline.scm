(struct_declaration
  "struct" @context
  name: (identifier) @name) @item

(field_declaration
  name: (identifier) @name) @item

(state_declaration
  "state" @context
  name: (identifier) @name) @item

(function_declaration
  "fn" @context
  name: (identifier) @name
  parameters: (parameter_list) @context.extra) @item

(view_declaration
  "view" @context
  name: (identifier) @name) @item

(source_file
  (expression_statement
    (call_expression
      function: (identifier) @name
      (#eq? @name "window")
      arguments: (argument_list
        .
        (string) @context.extra))) @item)

(named_element
  element: (identifier) @context
  name: (identifier) @name
  arguments: (argument_list
    .
    (string) @context.extra)) @item
