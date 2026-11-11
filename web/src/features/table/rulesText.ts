/**
 * Rules text in pieces for the preview (pure): the symbols in braces (`{2}`, `{G}`, `{W/P}`, `{T}`) as mana symbols or
 * the tap and untap marks, the word "target" with what follows it to the end of its phrase as a target, and plain text
 * between them. A symbol this page cannot draw stays text.
 */
import type { Color, ManaSymbol } from '@/protocol/generated'
import { costPips } from './cardModel'
import type { ManaPip } from './cardModel'

/** One piece of rules text. */
export type RulesPiece =
  | { kind: 'text'; text: string }
  | { kind: 'mana'; pip: ManaPip; text: string }
  | { kind: 'tap'; text: string }
  | { kind: 'untap'; text: string }
  | { kind: 'target'; text: string }

const COLOR: Record<string, Color> = { W: 'White', U: 'Blue', B: 'Black', R: 'Red', G: 'Green' }

/** The mana symbol a brace holds ("2", "G", "W/U", "2/W", "G/P", "C", "S"), or null for anything else. */
export function manaSymbolOf(inner: string): ManaSymbol | null {
  if (/^\d+$/.test(inner)) return { Generic: Number(inner) }
  if (inner === 'C') return 'Colorless'
  if (inner === 'S') return 'Snow'
  const one = COLOR[inner]
  if (one !== undefined) return { Colored: one }
  const parts = inner.split('/')
  if (parts.length === 2) {
    const [a = '', b = ''] = parts
    const ca = COLOR[a]
    const cb = COLOR[b]
    if (ca !== undefined && cb !== undefined) return { Hybrid: [ca, cb] }
    if (a === '2' && cb !== undefined) return { MonoHybrid: cb }
    if (ca !== undefined && b === 'P') return { Phyrexian: ca }
    if (a === 'C' && cb !== undefined) return { ColorlessHybrid: cb }
    if (a === 'C' && b === 'P') return 'ColorlessPhyrexian'
  }
  if (parts.length === 3 && parts[2] === 'P') {
    const ca = COLOR[parts[0] ?? '']
    const cb = COLOR[parts[1] ?? '']
    if (ca !== undefined && cb !== undefined) return { HybridPhyrexian: [ca, cb] }
  }
  return null
}

/** The pieces of plain text: each "target …" phrase (up to a comma, period or the end) apart from the words around it. */
function targets(text: string): RulesPiece[] {
  const out: RulesPiece[] = []
  const pattern = /\btargets?\b[^,.;:()]*/gi
  let at = 0
  for (const match of text.matchAll(pattern)) {
    const start = match.index
    if (start > at) out.push({ kind: 'text', text: text.slice(at, start) })
    out.push({ kind: 'target', text: match[0] })
    at = start + match[0].length
  }
  if (at < text.length) out.push({ kind: 'text', text: text.slice(at) })
  return out
}

/** `text` in pieces, in order; joining their `text` gives `text` back. */
export function rulesPieces(text: string): RulesPiece[] {
  const out: RulesPiece[] = []
  let at = 0
  for (const match of text.matchAll(/\{([^{}]+)\}/g)) {
    const start = match.index
    if (start > at) out.push(...targets(text.slice(at, start)))
    const inner = match[1] ?? ''
    const symbol = inner === 'T' || inner === 'Q' ? null : manaSymbolOf(inner)
    if (inner === 'T') out.push({ kind: 'tap', text: match[0] })
    else if (inner === 'Q') out.push({ kind: 'untap', text: match[0] })
    else if (symbol !== null) {
      const [pip] = costPips([symbol])
      if (pip !== undefined) out.push({ kind: 'mana', pip, text: match[0] })
    } else out.push({ kind: 'text', text: match[0] })
    at = start + match[0].length
  }
  if (at < text.length) out.push(...targets(text.slice(at)))
  return out
}

/** An ability's cost and effect apart, split at the first colon outside parentheses; no cost when there is none. */
export function splitCost(text: string): { cost: string; effect: string } {
  let depth = 0
  for (let i = 0; i < text.length; i += 1) {
    const char = text[i]
    if (char === '(') depth += 1
    else if (char === ')') depth -= 1
    else if (char === ':' && depth === 0)
      return { cost: text.slice(0, i), effect: text.slice(i + 1).trimStart() }
  }
  return { cost: '', effect: text }
}
