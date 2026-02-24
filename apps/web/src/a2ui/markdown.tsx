import type { ReactElement, ReactNode } from "react";

import { DEFAULT_RENDER_INPUT_LIMITS } from "./constants";
import { clampText, sanitizeExternalUrl } from "./sanitize";

interface SanitizedMarkdownProps {
  readonly value: string;
}

const INLINE_TOKEN_REGEX = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+]\(([^)]+)\))/g;

export function SanitizedMarkdown({ value }: SanitizedMarkdownProps): ReactElement {
  const bounded = clampText(value, DEFAULT_RENDER_INPUT_LIMITS.maxMarkdownLength);
  const normalized = bounded.replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  const blocks: ReactNode[] = [];

  let cursor = 0;
  while (cursor < lines.length) {
    const line = lines[cursor].trimEnd();
    if (line.trim().length === 0) {
      cursor += 1;
      continue;
    }

    const headingMatch = line.match(/^(#{1,4})\s+(.+)$/);
    if (headingMatch !== null) {
      const level = headingMatch[1].length;
      const text = headingMatch[2];
      blocks.push(renderHeading(level, text, cursor));
      cursor += 1;
      continue;
    }

    const unorderedItems = collectList(lines, cursor, /^\s*[-*]\s+(.+)$/);
    if (unorderedItems.items.length > 0) {
      blocks.push(
        <ul key={`ul-${cursor}`} className="a2ui-markdown-list">
          {unorderedItems.items.map((item, index) => (
            <li key={`${cursor}-${index}`}>{renderInline(item, `${cursor}-${index}`)}</li>
          ))}
        </ul>
      );
      cursor = unorderedItems.nextCursor;
      continue;
    }

    const orderedItems = collectList(lines, cursor, /^\s*\d+\.\s+(.+)$/);
    if (orderedItems.items.length > 0) {
      blocks.push(
        <ol key={`ol-${cursor}`} className="a2ui-markdown-list">
          {orderedItems.items.map((item, index) => (
            <li key={`${cursor}-${index}`}>{renderInline(item, `${cursor}-${index}`)}</li>
          ))}
        </ol>
      );
      cursor = orderedItems.nextCursor;
      continue;
    }

    blocks.push(
      <p key={`p-${cursor}`} className="a2ui-markdown-paragraph">
        {renderInline(line, `p-${cursor}`)}
      </p>
    );
    cursor += 1;
  }

  if (blocks.length === 0) {
    blocks.push(
      <p key="empty" className="a2ui-markdown-empty">
        No content.
      </p>
    );
  }

  return <div className="a2ui-markdown">{blocks}</div>;
}

function renderHeading(level: number, text: string, keySeed: number): ReactElement {
  const nodes = renderInline(text, `h-${keySeed}`);
  if (level === 1) {
    return (
      <h1 key={`h1-${keySeed}`} className="a2ui-markdown-heading">
        {nodes}
      </h1>
    );
  }
  if (level === 2) {
    return (
      <h2 key={`h2-${keySeed}`} className="a2ui-markdown-heading">
        {nodes}
      </h2>
    );
  }
  if (level === 3) {
    return (
      <h3 key={`h3-${keySeed}`} className="a2ui-markdown-heading">
        {nodes}
      </h3>
    );
  }
  return (
    <h4 key={`h4-${keySeed}`} className="a2ui-markdown-heading">
      {nodes}
    </h4>
  );
}

function renderInline(value: string, keySeed: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let lastIndex = 0;
  INLINE_TOKEN_REGEX.lastIndex = 0;

  for (const match of value.matchAll(INLINE_TOKEN_REGEX)) {
    const token = match[0];
    const index = match.index ?? 0;
    if (index > lastIndex) {
      nodes.push(value.slice(lastIndex, index));
    }

    if (token.startsWith("`") && token.endsWith("`")) {
      nodes.push(
        <code key={`code-${keySeed}-${index}`} className="a2ui-markdown-code">
          {token.slice(1, -1)}
        </code>
      );
      lastIndex = index + token.length;
      continue;
    }

    if (token.startsWith("**") && token.endsWith("**")) {
      nodes.push(<strong key={`strong-${keySeed}-${index}`}>{token.slice(2, -2)}</strong>);
      lastIndex = index + token.length;
      continue;
    }

    if (token.startsWith("*") && token.endsWith("*")) {
      nodes.push(<em key={`em-${keySeed}-${index}`}>{token.slice(1, -1)}</em>);
      lastIndex = index + token.length;
      continue;
    }

    const linkMatch = token.match(/^\[([^\]]+)]\(([^)]+)\)$/);
    if (linkMatch !== null) {
      const label = linkMatch[1];
      const href = sanitizeExternalUrl(linkMatch[2]);
      if (href === null) {
        nodes.push(<span key={`link-fallback-${keySeed}-${index}`}>{label}</span>);
      } else {
        nodes.push(
          <a
            key={`link-${keySeed}-${index}`}
            href={href}
            rel="noreferrer noopener"
            target="_blank"
          >
            {label}
          </a>
        );
      }
      lastIndex = index + token.length;
      continue;
    }

    nodes.push(token);
    lastIndex = index + token.length;
  }

  if (lastIndex < value.length) {
    nodes.push(value.slice(lastIndex));
  }

  return nodes;
}

function collectList(
  lines: readonly string[],
  start: number,
  pattern: RegExp
): { items: string[]; nextCursor: number } {
  const items: string[] = [];
  let cursor = start;

  while (cursor < lines.length) {
    const line = lines[cursor];
    const match = line.match(pattern);
    if (match === null) {
      break;
    }
    items.push(match[1].trimEnd());
    cursor += 1;
  }

  return {
    items,
    nextCursor: cursor
  };
}
