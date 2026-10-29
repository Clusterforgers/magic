/**
 * Why a card of the viewer's hand cannot be played now (docs/ui-design.md, Playing is legible). Pure: it reads the
 * card, the table and the priority offer. The server's reason (`PriorityOptions.unplayable`, ui1a) replaces this
 * guess once it is sent; until then the timing, the land played and too little mana are read off public state.
 */
import type { ManaCost, ObjectView, PriorityOptions, StateView } from '@/protocol/generated'
import type { CardNote, CardNotes } from './prompt/marks'
import { manaValue } from './prompt/payment'

/** A reason the server can give. */
export type Unplayable = 'Timing' | 'LandPlayed' | 'Mana' | 'Targets' | 'Cost' | 'Restricted'

/** A reason the hand shows: one the server or the guess gives, or `Unknown` when neither knows. */
export type HandReason = Unplayable | 'Unknown'

/** The short label on the card's foot and the line of the preview, for each reason. */
export const REASON_WORDS: Readonly<Record<HandReason, { label: string; line: string }>> = {
  Timing: { label: 'Sorcery speed', line: 'Only in your main phase with an empty stack' },
  LandPlayed: { label: 'Land played', line: 'You have played a land this turn' },
  Mana: { label: 'Needs mana', line: 'Not enough mana' },
  Targets: { label: 'No targets', line: 'Nothing it can target' },
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
  ])
}

/** Whether the viewer could act at sorcery speed now: their own main phase with an empty stack. */
export function sorceryWindow(view: StateView): boolean {
  const { turn, stack, viewer } = view
  return turn.active === viewer && (turn.step === 'Main1' || turn.step === 'Main2') && stack.count === 0
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
 * The note of every card in the viewer's hand that cannot be played now: its reason's label drawn on hover and focus,
 * and the same words for a screen reader.
 */
export function handNotes(
  view: StateView,
  objectsById: Readonly<Record<number, ObjectView>>,
  options: PriorityOptions,
): CardNotes {
  const notes = new Map<number, CardNote>()
  for (const id of view.players[view.viewer]?.hand.cards ?? []) {
    const object = objectsById[id]
    const reason = object === undefined ? null : unplayableReason(object, view, options)
    if (reason === null) continue
    const { label } = REASON_WORDS[reason]
    notes.set(id, { hint: label, words: label.toLowerCase() })
  }
  return notes
}

const costOf = (cost: ManaCost | null): number => (cost === null ? 0 : manaValue(cost))

/**
 * Why `object`, a card in the viewer's hand, cannot be played now; null when the offer lets it be played. A land
 * waits for the viewer's main phase and their one land; a spell without Flash waits for the main phase, and one that
 * costs more than all the mana at hand needs mana. Anything else is `Unknown`.
 */
export function unplayableReason(
  object: ObjectView,
  view: StateView,
  options: PriorityOptions,
): HandReason | null {
  if (offered(options).has(object.id)) return null
  const sorcery = sorceryWindow(view)
  if (object.types.includes('Land')) {
    if (!sorcery) return 'Timing'
    return view.turn.lands_played > 0 ? 'LandPlayed' : 'Unknown'
  }
  const instant = object.types.includes('Instant') || (object.keywords ?? []).includes('Flash')
  if (!instant && !sorcery) return 'Timing'
  if (costOf(object.mana_cost) > manaAtHand(view, options)) return 'Mana'
  return 'Unknown'
}
