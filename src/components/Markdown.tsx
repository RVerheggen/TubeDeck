import type { ReactNode } from "react";

// Just enough Markdown for CHANGELOG.md: "### " headings, "- " bullets (with indented
// continuation lines), paragraphs, **bold** and `code`. Rendered as React elements, never
// as HTML, so text from the network can't inject markup.

function inline(text: string): ReactNode[] {
  return text.split(/(\*\*[^*]+\*\*|`[^`]+`)/g).map((part, i) => {
    if (part.startsWith("**") && part.endsWith("**") && part.length > 4) return <strong key={i}>{part.slice(2, -2)}</strong>;
    if (part.startsWith("`") && part.endsWith("`") && part.length > 2) return <code key={i}>{part.slice(1, -1)}</code>;
    return part;
  });
}

export function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  let items: string[] = [];
  let para: string[] = [];
  const flushList = () => {
    if (items.length) blocks.push(<ul key={blocks.length}>{items.map((t, i) => <li key={i}>{inline(t)}</li>)}</ul>);
    items = [];
  };
  const flushPara = () => {
    if (para.length) blocks.push(<p key={blocks.length}>{inline(para.join(" "))}</p>);
    para = [];
  };
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trimEnd();
    if (!line.trim()) {
      flushList();
      flushPara();
    } else if (line.startsWith("#")) {
      flushList();
      flushPara();
      blocks.push(<h4 key={blocks.length}>{inline(line.replace(/^#+\s*/, ""))}</h4>);
    } else if (/^\s*[-*] /.test(line)) {
      flushPara();
      items.push(line.replace(/^\s*[-*] /, ""));
    } else if (items.length && /^\s+/.test(raw)) {
      items[items.length - 1] += ` ${line.trim()}`;
    } else {
      flushList();
      para.push(line.trim());
    }
  }
  flushList();
  flushPara();
  return <div className="markdown">{blocks}</div>;
}
