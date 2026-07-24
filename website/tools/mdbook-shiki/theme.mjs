// A Shiki theme wired to the superui palette (see theme/css/site.css). Cool hues
// (purple/teal/blue) carry code structure; warm amber carries literals — the same
// split the landing page's hand-rolled code sample uses (.k/.fn/.tag/.attr/.n).
//
// The `bg` is dropped from the <pre> by the preprocessor so the site's own
// `.content pre` panel background/border shows through; the default `fg` matches
// `--su-text`, so untokenized text blends with body code color.
export const theme = {
  name: "superui-teal",
  type: "dark",
  fg: "#cdd8e6",
  bg: "transparent",
  colors: {
    "editor.foreground": "#cdd8e6",
    "editor.background": "#00000000",
  },
  settings: [
    { scope: ["comment", "punctuation.definition.comment"],
      settings: { foreground: "#5f7085", fontStyle: "italic" } },

    // keywords / storage / language constants → purple (matches landing `.k`)
    { scope: [
        "keyword", "storage", "storage.type", "storage.modifier",
        "keyword.control", "constant.language", "variable.language",
        "keyword.other", "meta.import keyword", "meta.export keyword",
      ],
      settings: { foreground: "#c58fff" } },

    // operators + punctuation → muted
    { scope: [
        "keyword.operator", "punctuation", "meta.brace",
        "punctuation.definition", "punctuation.separator",
        "punctuation.terminator", "punctuation.accessor",
      ],
      settings: { foreground: "#8a97a8" } },

    // strings → amber-light
    { scope: [
        "string", "string.template", "string.quoted",
        "punctuation.definition.string", "constant.other.symbol",
      ],
      settings: { foreground: "#ffce8a" } },

    // numbers / boolean / null → amber
    { scope: [
        "constant.numeric", "constant.language.boolean",
        "constant.language.null", "constant.language.undefined",
        "support.constant", "constant.other",
      ],
      settings: { foreground: "#ffb454" } },

    // function names → teal-light (matches landing `.fn`)
    { scope: [
        "entity.name.function", "support.function",
        "meta.function-call.method", "variable.function",
      ],
      settings: { foreground: "#7ff3e9" } },

    // types / classes → light blue
    { scope: [
        "entity.name.type", "support.type", "support.class",
        "entity.name.class", "entity.other.inherited-class",
      ],
      settings: { foreground: "#5fd7ff" } },

    // JSX/HTML tag names → light blue (matches landing `.tag`)
    { scope: ["entity.name.tag", "support.class.component"],
      settings: { foreground: "#5fd7ff" } },

    // JSX/HTML attribute names → teal (matches landing `.attr`)
    { scope: ["entity.other.attribute-name"],
      settings: { foreground: "#34e6d6" } },

    // identifiers / properties / params → default body code color
    { scope: [
        "variable", "variable.other", "variable.parameter",
        "meta.definition.variable", "variable.other.property",
        "meta.object-literal.key", "support.type.property-name",
      ],
      settings: { foreground: "#cdd8e6" } },
  ],
};
