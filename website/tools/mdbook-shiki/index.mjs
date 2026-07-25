#!/usr/bin/env node
// mdBook preprocessor: highlight fenced code blocks at build time with Shiki,
// themed to the superui palette. mdBook's own (client-side) highlight.js is
// neutralized by theme/highlight.js so it never re-processes this markup.
//
// Protocol: `<cmd> supports <renderer>` must exit 0/1; otherwise mdBook pipes
// `[context, book]` as JSON on stdin and expects the modified `book` on stdout.
import process from "node:process";
import { createHighlighter } from "shiki";
import { theme } from "./theme.mjs";

// "supports" probe — we handle every renderer (highlighting is renderer-agnostic).
if (process.argv[2] === "supports") process.exit(0);

// Languages we actually use in the docs. Kept small so highlighter init is fast.
const LANGS = [
  "tsx", "typescript", "javascript", "jsx",
  "rust", "toml", "html", "css", "json", "bash", "ini",
];
// Fence tag → Shiki language id. Note: `typescript`/`ts` are routed to the `tsx`
// grammar (a superset) so our JSX-bearing authoring snippets highlight fully
// without needing every fence relabelled to ```tsx.
const ALIAS = {
  sh: "bash", shell: "bash", console: "bash",
  jsonc: "json", typescript: "tsx", ts: "tsx", js: "javascript",
  gitignore: "ini", plaintext: "text", txt: "text", "": "text",
};

const escapeHtml = (s) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

// Fenced code blocks at line start: ```lang\n…\n```
const FENCE = /(^|\n)```([^\n`]*)\n([\s\S]*?)\n```/g;

// Indented fences (```lang inside a list item) can't be turned into a highlighted
// <pre> without either breaking the list nesting or corrupting copy-paste, so we
// only handle column-0 fences. Warn if the source has an indented one so it gets
// restructured rather than silently rendering unhighlighted.
const INDENTED_FENCE = /^[ \t]+```[^\n`]/m;

function warnIfIndentedFence(md, chapter) {
  if (INDENTED_FENCE.test(md)) {
    process.stderr.write(
      `mdbook-shiki: warning: indented code fence in "${chapter}" won't be ` +
      `highlighted — move it to column 0 (out of the list item).\n`);
  }
}

function highlightMarkdown(md, hl) {
  return md.replace(FENCE, (_m, lead, info, code) => {
    const tag = (info.trim().split(/\s+/)[0] || "").toLowerCase();
    const lang = ALIAS[tag] ?? tag;
    let html;
    try {
      html = hl.codeToHtml(code, { lang, theme: theme.name });
    } catch {
      // Unknown/unloaded language → plain, still inside a .shiki panel.
      html = `<pre class="shiki ${theme.name}" tabindex="0"><code>${escapeHtml(code)}</code></pre>`;
    }
    // Drop the <pre>'s inline background so the site's .content pre panel shows.
    html = html.replace(/(<pre\b[^>]*?)\s+style="[^"]*"/, "$1");
    // Blank lines around the raw HTML so the markdown renderer treats it as a block.
    return `${lead}\n${html}\n`;
  });
}

function walkSections(items, hl) {
  for (const item of items) {
    if (item && item.Chapter) {
      warnIfIndentedFence(item.Chapter.content, item.Chapter.name || "?");
      item.Chapter.content = highlightMarkdown(item.Chapter.content, hl);
      if (item.Chapter.sub_items) walkSections(item.Chapter.sub_items, hl);
    }
  }
}

function readStdin() {
  return new Promise((resolve, reject) => {
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (c) => (data += c));
    process.stdin.on("end", () => resolve(data));
    process.stdin.on("error", reject);
  });
}

async function main() {
  const input = await readStdin();
  const [, book] = JSON.parse(input);
  const hl = await createHighlighter({ themes: [theme], langs: LANGS });
  walkSections(book.items, hl); // mdBook's Book serializes its chapters as `items`
  process.stdout.write(JSON.stringify(book));
}

main().catch((err) => {
  console.error("mdbook-shiki:", err && err.stack ? err.stack : err);
  process.exit(1);
});
