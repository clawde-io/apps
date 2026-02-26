// ClawDE editor theme for CodeMirror 6 (Sprint HH, ED.11).
// Brand: bg #0f0f14, cursor #dc2626, selection #7f1d1d, amber accent #ff784e

import { EditorView } from '@codemirror/view';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';

// ─── Editor view theme ────────────────────────────────────────────────────────

export const clawdTheme = EditorView.theme(
  {
    '&': {
      color: '#e5e7eb',
      backgroundColor: '#0f0f14',
      height: '100%',
    },
    '.cm-content': {
      caretColor: '#dc2626',
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      fontSize: '13px',
      lineHeight: '1.6',
    },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: '#dc2626', borderLeftWidth: '2px' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
      backgroundColor: '#7f1d1d',
    },
    '.cm-activeLine': { backgroundColor: '#1a1a1f' },
    '.cm-activeLineGutter': { backgroundColor: '#1a1a1f' },
    '.cm-gutters': {
      backgroundColor: '#0d0d12',
      color: '#4b5563',
      border: 'none',
      borderRight: '1px solid #1f2937',
    },
    '.cm-lineNumbers .cm-gutterElement': { paddingRight: '12px' },
    '.cm-foldGutter .cm-gutterElement': { color: '#6b7280' },
    '.cm-matchingBracket': {
      backgroundColor: '#374151',
      outline: '1px solid #6b7280',
    },
    '.cm-tooltip': {
      backgroundColor: '#111118',
      border: '1px solid #1f2937',
      color: '#e5e7eb',
    },
    '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': {
      backgroundColor: '#dc2626',
      color: '#fff',
    },
    '.cm-searchMatch': { backgroundColor: '#7f1d1d', outline: '1px solid #dc2626' },
    '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: '#dc2626' },
    '.cm-panels': { backgroundColor: '#0d0d12', color: '#e5e7eb' },
    '.cm-panels.cm-panels-top': { borderBottom: '1px solid #1f2937' },
    '.cm-panels.cm-panels-bottom': { borderTop: '1px solid #1f2937' },
    // Ghost text (completion overlay)
    '.cm-ghostText, .cm-ghostText *': { opacity: 0.45, fontStyle: 'italic' },
  },
  { dark: true }
);

// ─── Syntax highlight style ───────────────────────────────────────────────────

export const clawdHighlightStyle = HighlightStyle.define([
  { tag: t.keyword, color: '#f87171' },          // red keywords
  { tag: [t.name, t.deleted, t.character, t.propertyName, t.macroName], color: '#93c5fd' },
  { tag: [t.function(t.variableName), t.labelName], color: '#67e8f9' },
  { tag: [t.color, t.constant(t.name), t.standard(t.name)], color: '#fca5a5' },
  { tag: [t.definition(t.name), t.separator], color: '#e5e7eb' },
  { tag: [t.typeName, t.className, t.number, t.changed, t.annotation, t.modifier, t.self, t.namespace], color: '#a78bfa' },
  { tag: [t.operator, t.operatorKeyword, t.url, t.escape, t.regexp, t.link, t.special(t.string)], color: '#ff784e' },
  { tag: [t.meta, t.comment], color: '#4b5563', fontStyle: 'italic' },
  { tag: t.strong, fontWeight: 'bold' },
  { tag: t.emphasis, fontStyle: 'italic' },
  { tag: t.strikethrough, textDecoration: 'line-through' },
  { tag: t.link, color: '#60a5fa', textDecoration: 'underline' },
  { tag: t.heading, fontWeight: 'bold', color: '#f9fafb' },
  { tag: [t.atom, t.bool, t.special(t.variableName)], color: '#fdba74' },
  { tag: [t.processingInstruction, t.string, t.inserted], color: '#86efac' },
  { tag: t.invalid, color: '#f87171', borderBottom: '1px solid #ef4444' },
]);

export const clawdSyntaxHighlighting = syntaxHighlighting(clawdHighlightStyle);

// ─── Combined extension ───────────────────────────────────────────────────────

export const clawdEditorTheme = [clawdTheme, clawdSyntaxHighlighting];
