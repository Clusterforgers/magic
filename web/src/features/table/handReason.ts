/**
 * Why a card of the viewer's hand, or another card the server names, cannot be played now (docs/ui-design.md, Playing is legible). Pure: it reads the
 * card, the table and the priority offer. The server's reason (`PriorityOptions.unplayable`) wins; for a card it does not
 * name, the timing, the land played and too little mana are read off public state.
 */
import type { ManaCost, ObjectView, PriorityOptions, StateView, Unplayable, Why } from '@/protocol/generated'
import { sharesTurn } from '@/lib/teams'
import type { Blocker, CardNote, CardNotes } from './prompt/marks'
import { manaValue } from './prompt/payment'
import { whyLine } from './whyWords'

/** A reason the server can give. */
export type { Unplayable }

/** A reason the hand shows: one the server or the guess gives, or `Unknown` when neither knows. */
export type HandReason = Unplayable | 'Unknown'

/** The short label on the card's foot and the line of the preview, for each reason. */
export const REASON_WORDS: Readonly<Record<HandReason, { label: string; line: string }>> = {
  Timing: { label: 'Sorcery speed', line: 'Sorcery speed: only in your main phase with an empty stack' },
  LandPlayed: { label: 'Land played', line: 'Land already played this turn' },
  Mana: { label: 'Needs mana', line: 'Not enough mana' },
  Targets: { label: 'No targets', line: 'No legal target' },
  Cost: { label: "Can't pay", line: 'A cost cannot be paid' },
  Restricted: { label: 'Not allowed', line: 'An effect forbids it now' },
  Unknown: { label: 'Not now', line: 'Not playable now' },
}

/** Every card the offer lets the holder play or use now. */
function offered(options: PriorityOptions): Set<number> {
  return new Set([
    ...options.castable,
    ...options.playable_lands,
    ...(options.faces ?? []).map((face) => face.card),
    ...(options.suspendable ?? []),
    ...(options.foretellable ?? []),
    ...(options.plottable ?? []),
  ])
}

/** Whether the viewer could act at sorcery speed now: a main phase of their turn (their team's in Two-Headed Giant) with an empty stack. */
export function sorceryWindow(view: StateView): boolean {
  const { turn, stack, viewer } = view
  return (
    sharesTurn(view.house_rules?.teams, viewer, turn.active) &&
    (turn.step === 'Main1' || turn.step === 'Main2') &&
    stack.count === 0
  )
}

/** The most mana the holder could have: the pool and one use of each permanent with a mana ability. */
export function manaAtHand(view: StateView, options: PriorityOptions): number {
  const pool = view.players[view.viewer]?.mana.counts.reduce((sum, count) => sum + count, 0) ?? 0
  const best = new Map<number, number>()
  for (const option of options.mana_abilities) {
    const { object } = option.source
    best.set(object, Math.max(best.get(object) ?? 0, option.count ?? 1))
  }
  return pool + [...best.values()].reduce((sum, count) => sum + count, 0)
}

/**
 * What blocks a card when a rule in force does: the object that put it in force (One Voice, Grand Abolisher) or the
 * split second spell on the stack; empty for any other reason.
 */
export function blockerOf(why: Why | undefined): Blocker {
  if (why === undefined || typeof why === 'string') return {}
  if ('ForbiddenBy' in why) return { source: why.ForbiddenBy.source, name: why.ForbiddenBy.name }
  if ('SplitSecond' in why) return { source: why.SplitSecond.spell, name: why.SplitSecond.name }
  return {}
}

/** The short label on the card's foot: the name of the rule that blocks it when there is one, else the reason's label. */
export function reasonLabel(reason: HandReason, why: Why | undefined): string {
  if (why !== undefined && typeof why !== 'string' && 'SplitSecond' in why) return 'Split second'
  return blockerOf(why).name ?? REASON_WORDS[reason].label
}

/**
 * The note of every card of the viewer's hand that cannot be played now, and of each other card the server says why
 * of (a commander in the command zone, a flashback card in the graveyard, a card in exile, the top of the library): the
 * reason's label drawn on hover and focus, the same words for a screen reader, the reason's line for the card's preview
 * and for a press (`blocked`), with what blocks it.
 */
export function handNotes(
  view: StateView,
  objectsById: Readonly<Record<number, ObjectView>>,
  options: PriorityOptions,
): CardNotes {
  const notes = new Map<number, CardNote>()
  const hand = view.players[view.viewer]?.hand.cards ?? []
  const told = new Map((options.unplayable ?? []).map((entry) => [entry.card, entry]))
  const ids = [...hand, ...[...told.keys()].filter((id) => !hand.includes(id))]
  for (const id of ids) {
    const object = objectsById[id]
    const reason = object === undefined ? null : unplayableReason(object, view, options)
    if (reason === null || object === undefined) continue
    const entry = told.get(id)
    const label = reasonLabel(reason, entry?.why)
    const blocked = blockerOf(entry?.why)
    if (entry?.why === undefined) {
      const { line } = REASON_WORDS[reason]
      notes.set(id, { hint: label, line, words: label.toLowerCase(), blocked })
      continue
    }
    const said = whyLine(
      { name: object.name, rulesText: object.rules_text, doing: 'cast' },
      entry.reason,
      entry.why,
    )
    notes.set(id, { hint: label, line: said, words: `${label.toLowerCase()}: ${said}`, blocked })
  }
  return notes
}

/**
 * The note of every permanent of the viewer with activated abilities, none of which can be activated now: the reason
 * of its first ability (`PriorityOptions.inactive`) in the preview and the spoken name, no hint on the card.
 */
export function inactiveNotes(
  objectsById: Readonly<Record<number, ObjectView>>,
  options: PriorityOptions,
): CardNotes {
  const notes = new Map<number, CardNote>()
  const usable = new Set((options.abilities ?? []).map((ability) => ability.object))
  for (const entry of options.inactive ?? []) {
    const id = entry.ability.object
    const object = objectsById[id]
    if (object === undefined || usable.has(id) || notes.has(id)) continue
    const line = whyLine(
      { name: object.name, rulesText: object.rules_text, doing: 'activate' },
      entry.reason,
      entry.why,
    )
    notes.set(id, { line, words: `can't activate now: ${line}` })
  }
  return notes
}

const costOf = (cost: ManaCost | null): number => (cost === null ? 0 : manaValue(cost))

/**
 * Why `object`, a card in the viewer's hand, cannot be played now; null when the offer lets it be played. The server's
 * reason comes first; without one, a land
 * waits for the viewer's main phase and their one land; a spell without Flash waits for the main phase, and one that
 * costs more than all the mana at hand needs mana. Anything else is `Unknown`.
 */
export function unplayableReason(
  object: ObjectView,
  view: StateView,
  options: PriorityOptions,
): HandReason | null {
  if (offered(options).has(object.id)) return null
  const told = options.unplayable?.find((entry) => entry.card === object.id)
  if (told !== undefined) return told.reason
  const sorcery = sorceryWindow(view)
  if (object.types.includes('Land')) {
    if (!sorcery) return 'Timing'
    return view.turn.lands_played > 0 ? 'LandPlayed' : 'Unknown'
  }
  const instant = object.types.includes('Instant') || (object.keywords ?? []).includes('Flash')
  if (!instant && !sorcery) return 'Timing'
  if (costOf(object.cost_now ?? object.mana_cost) > manaAtHand(view, options)) return 'Mana'
  return 'Unknown'
}
