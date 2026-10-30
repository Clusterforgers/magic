/**
 * The marks of a batch as effects and arrows (docs/ui-design.md, Playing is legible). Pure: `deriveMoves` names what
 * happened to which object, this keeps each mark once, caps them and picks the ones that get an arrow.
 */
import type { ObjectId } from '@/protocol/generated'
import { MARK_FX } from './model'
import type { MarkEffect, MarkFx } from './model'
import type { MarkKind, MoveMark } from './moves'

/** The most marks one batch shows; a board wipe stays readable and cheap. */
export const MAX_MARKS = 12

/** The most arrows one batch draws from resolving objects. */
export const MAX_MARK_ARROWS = 4

/** The short word of each mark, beside its icon. */
export const MARK_WORDS: Readonly<Record<MarkFx, string>> = {
  activated: 'Activated',
  triggered: 'Triggered',
  resolved: 'Resolved',
  countered: 'Countered',
  fizzled: 'Fizzled',
  exiled: 'Exiled',
  bounced: 'Bounced',
  drawn: 'Drawn',
}

/** What a mark's badge needs besides its word in px: border, padding, icon and the gap after it. */
const MARK_CHROME_PX = 34

/** The width of a letter of a mark's word in px (12 px, semibold). */
const MARK_CHAR_PX = 7

/** The room a badge leaves at the edges of a card in px (`fx.css`). */
const MARK_MARGIN_PX = 4

/** Whether a badge of `kind` can write its word on a card `cardWidth` px wide; a narrower card (a hand card) gets the icon alone. */
export function markShowsWord(kind: MarkFx, cardWidth: number): boolean {
  return cardWidth - MARK_MARGIN_PX >= MARK_CHROME_PX + MARK_WORDS[kind].length * MARK_CHAR_PX
}

const shown = (kind: MarkKind): kind is MarkFx => (MARK_FX as readonly string[]).includes(kind)

/** The marks to show: those with a face, each kind once on an object, the first `MAX_MARKS`. */
export function markEffects(marks: readonly MoveMark[]): MarkEffect[] {
  const seen = new Set<string>()
  const effects: MarkEffect[] = []
  for (const mark of marks) {
    if (!shown(mark.kind)) continue
    const key = `${mark.kind}:${mark.object}`
    if (seen.has(key)) continue
    seen.add(key)
    effects.push({ kind: mark.kind, object: mark.object })
  }
  return effects.slice(0, MAX_MARKS)
}

/** What a resolving object does to another: it counters, exiles, bounces or destroys it. */
export interface MarkArrow {
  readonly by: ObjectId
  readonly object: ObjectId
  readonly kind: 'countered' | 'exiled' | 'bounced' | 'died'
}

const POINTING = new Set<MarkKind>(['countered', 'exiled', 'bounced', 'died'])

/** The arrows from the resolving object to what it counters, exiles, bounces or destroys; each pair once. */
export function markArrows(marks: readonly MoveMark[]): MarkArrow[] {
  const seen = new Set<string>()
  const arrows: MarkArrow[] = []
  for (const { kind, object, by } of marks) {
    if (by === undefined || by === object || !POINTING.has(kind)) continue
    const key = `${by}:${object}`
    if (seen.has(key)) continue
    seen.add(key)
    arrows.push({ by, object, kind: kind as MarkArrow['kind'] })
  }
  return arrows.slice(0, MAX_MARK_ARROWS)
}
