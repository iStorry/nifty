// mdBook's bundled highlight.js has no Nifty grammar, and book.js highlights every block
// before additional scripts load, so the `nf` blocks are highlighted again once it exists.
(function () {
  "use strict";

  const ELEMENTS = [
    "window", "column", "row", "label", "button",
    "text_field", "checkbox", "spacer", "divider",
  ];
  const FUNCTIONS = [
    "download", "pick_file", "pick_files", "pick_folder", "pick_save_path",
  ];
  const METHODS = ["len", "is_empty", "push", "remove"];

  function nifty(hljs) {
    const number = { className: "number", begin: /\b\d[\d_]*(\.\d+)?\b/ };
    const interpolation = {
      className: "subst",
      begin: /\{/,
      end: /\}/,
      keywords: { literal: "true false" },
    };
    const string = {
      className: "string",
      begin: /"/,
      end: /"/,
      contains: [hljs.BACKSLASH_ESCAPE, interpolation],
    };
    interpolation.contains = [string, number];

    return {
      name: "Nifty",
      aliases: ["nifty"],
      keywords: {
        keyword: "state let fn view struct if else for in return",
        literal: "true false",
        type: "int float bool string",
        built_in: ELEMENTS.concat(FUNCTIONS).join(" "),
      },
      contains: [
        hljs.C_LINE_COMMENT_MODE,
        string,
        number,
        {
          className: "function",
          beginKeywords: "fn view struct",
          end: /[({]/,
          excludeEnd: true,
          contains: [hljs.UNDERSCORE_TITLE_MODE],
        },
        {
          className: "built_in",
          begin: new RegExp("(?<=\\.)(" + METHODS.join("|") + ")(?=\\s*\\()"),
        },
        { className: "variable", begin: /\$[A-Za-z_][A-Za-z0-9_]*/ },
        { className: "symbol", begin: /(?<![A-Za-z0-9_)\]])\.[a-z_][a-z0-9_]*/ },
        { className: "type", begin: /\b[A-Z][A-Za-z0-9_]*\b/ },
      ],
    };
  }

  hljs.registerLanguage("nf", nifty);
  document.querySelectorAll("code.language-nf").forEach(function (block) {
    hljs.highlightBlock(block);
  });
})();
