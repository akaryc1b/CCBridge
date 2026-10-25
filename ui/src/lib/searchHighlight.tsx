import { Fragment } from "react";
/** Literal, Unicode-safe highlighting: source content is always rendered as text. */
export function searchHighlight(text: string, query: string) {
  const q = query.trim();
  if (!q) return text;
  const literal = q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return text
    .split(new RegExp(`(${literal})`, "giu"))
    .map((part, index) =>
      index % 2 ? (
        <mark key={index}>{part}</mark>
      ) : (
        <Fragment key={index}>{part}</Fragment>
      ),
    );
}
