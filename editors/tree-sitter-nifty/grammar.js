/**
 * @file Tree-sitter grammar for the Nifty language.
 *
 * Mirrors `crates/nifty-syntax`: newlines end statements, so they are tokens here rather
 * than extras, and the grammar allows them only where the parser skips them.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const PREC = {
  or: 1,
  and: 2,
  compare: 3,
  add: 4,
  multiply: 5,
  unary: 6,
  postfix: 7,
};

/** Zero or more line breaks, where the language lets an expression continue. */
const newlines = ($) => repeat($._newline);

/** `open item (, item)* ,? close`, with line breaks free between the brackets. */
function bracketed($, open, item, close) {
  return seq(
    open,
    newlines($),
    optional(
      seq(
        item,
        repeat(seq(newlines($), ",", newlines($), item)),
        newlines($),
        optional(seq(",", newlines($))),
      ),
    ),
    close,
  );
}

/** `item (line-break+ item)*` with leading and trailing line breaks, as in a block. */
function lines($, item) {
  return seq(
    newlines($),
    optional(seq(item, repeat(seq(repeat1($._newline), item)), newlines($))),
  );
}

module.exports = grammar({
  name: "nifty",

  word: ($) => $.identifier,

  extras: ($) => [/[ \t\r]/, $.comment],

  conflicts: ($) => [
    // `if ready() { … }`: the `{` opens the `if` body, never a trailing block. The
    // trailing-block parse fails there, so the GLR parser keeps the other one.
    [$.call_expression],
  ],

  rules: {
    source_file: ($) => lines($, $._item),

    _item: ($) =>
      choice(
        $.state_declaration,
        $.struct_declaration,
        $.function_declaration,
        $.view_declaration,
        $._statement,
      ),

    state_declaration: ($) =>
      seq(
        "state",
        field("name", $.identifier),
        optional(seq(":", field("type", $._type))),
        "=",
        field("value", $._expression),
      ),

    struct_declaration: ($) =>
      seq("struct", field("name", $.identifier), field("body", $.field_declaration_list)),

    field_declaration_list: ($) =>
      seq(
        "{",
        newlines($),
        optional(
          seq(
            $.field_declaration,
            repeat(seq($._field_separator, $.field_declaration)),
            optional($._field_separator),
          ),
        ),
        "}",
      ),

    _field_separator: ($) => choice(seq(",", newlines($)), repeat1($._newline)),

    field_declaration: ($) => seq(field("name", $.identifier), ":", field("type", $._type)),

    function_declaration: ($) =>
      seq(
        "fn",
        field("name", $.identifier),
        field("parameters", $.parameter_list),
        optional(seq("->", field("return_type", $._type))),
        field("body", $.block),
      ),

    view_declaration: ($) =>
      seq(
        "view",
        field("name", $.identifier),
        optional(field("parameters", $.parameter_list)),
        field("body", $.block),
      ),

    parameter_list: ($) => bracketed($, "(", $.parameter, ")"),

    parameter: ($) => seq(field("name", $.identifier), ":", field("type", $._type)),

    _type: ($) => choice(alias($.identifier, $.type_identifier), $.list_type),

    list_type: ($) => seq("[", field("element", $._type), "]"),

    block: ($) => seq("{", lines($, $._statement), "}"),

    _statement: ($) =>
      choice(
        $.let_statement,
        $.if_statement,
        $.for_statement,
        $.return_statement,
        $.assignment_statement,
        $.expression_statement,
      ),

    let_statement: ($) =>
      seq(
        "let",
        field("name", $.identifier),
        optional(seq(":", field("type", $._type))),
        "=",
        field("value", $._expression),
      ),

    if_statement: ($) =>
      seq(
        "if",
        field("condition", $._expression),
        field("consequence", $.block),
        optional(seq("else", field("alternative", choice($.block, $.if_statement)))),
      ),

    for_statement: ($) =>
      seq(
        "for",
        field("item", $.identifier),
        "in",
        field("iterable", $._expression),
        field("body", $.block),
      ),

    return_statement: ($) => prec.right(seq("return", optional(field("value", $._expression)))),

    assignment_statement: ($) =>
      seq(
        field("target", $._expression),
        field("operator", choice("=", "+=", "-=", "*=", "/=")),
        newlines($),
        field("value", $._expression),
      ),

    expression_statement: ($) => $._expression,

    _expression: ($) =>
      choice(
        $.binary_expression,
        $.unary_expression,
        $.call_expression,
        $.field_expression,
        $.index_expression,
        $.parenthesized_expression,
        $.list_expression,
        $.implicit_member_expression,
        $.string,
        $.integer,
        $.float,
        $.boolean,
        $.identifier,
      ),

    binary_expression: ($) => {
      const table = [
        [PREC.or, "||"],
        [PREC.and, "&&"],
        [PREC.compare, choice("==", "!=", "<", "<=", ">", ">=")],
        [PREC.add, choice("+", "-")],
        [PREC.multiply, choice("*", "/", "%")],
      ];
      return choice(
        ...table.map(([precedence, operator]) =>
          prec.left(
            precedence,
            seq(
              field("left", $._expression),
              field("operator", operator),
              newlines($),
              field("right", $._expression),
            ),
          ),
        ),
      );
    },

    unary_expression: ($) =>
      prec(
        PREC.unary,
        seq(field("operator", choice("!", "-", "$")), field("operand", $._expression)),
      ),

    call_expression: ($) =>
      prec(
        PREC.postfix,
        seq(
          field("function", $._expression),
          field("arguments", $.argument_list),
          optional(field("trailing_block", $.block)),
        ),
      ),

    argument_list: ($) => bracketed($, "(", choice($.named_argument, $._expression), ")"),

    named_argument: ($) =>
      seq(field("label", $.identifier), ":", newlines($), field("value", $._expression)),

    field_expression: ($) =>
      prec(PREC.postfix, seq(field("value", $._expression), ".", field("field", $.identifier))),

    index_expression: ($) =>
      prec(
        PREC.postfix,
        seq(
          field("value", $._expression),
          "[",
          newlines($),
          field("index", $._expression),
          newlines($),
          "]",
        ),
      ),

    parenthesized_expression: ($) =>
      seq("(", newlines($), $._expression, newlines($), ")"),

    list_expression: ($) => bracketed($, "[", $._expression, "]"),

    implicit_member_expression: ($) => seq(".", field("member", $.identifier)),

    string: ($) =>
      seq(
        '"',
        repeat(choice($.string_content, $.escape_sequence, $.interpolation)),
        token.immediate('"'),
      ),

    string_content: (_) => token.immediate(prec(1, /[^"{\\\n]+/)),

    escape_sequence: (_) => token.immediate(/\\./),

    interpolation: ($) =>
      seq(token.immediate("{"), newlines($), $._expression, newlines($), "}"),

    integer: (_) => /[0-9][0-9_]*/,

    float: (_) => /[0-9][0-9_]*\.[0-9][0-9_]*/,

    boolean: (_) => choice("true", "false"),

    identifier: (_) => /[A-Za-z_][A-Za-z0-9_]*/,

    comment: (_) => token(seq("//", /[^\n]*/)),

    _newline: (_) => "\n",
  },
});
