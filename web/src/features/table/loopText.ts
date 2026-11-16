import type { LoopStep, LoopSummary, PlayerId } from '@/protocol/generated'

/** The numbers of iterations the loop prompt offers as buttons; the field takes any other. */
export const LOOP_PRESETS = [10, 100, 1000] as const

/** Names a seat. */
type Namer = (seat: PlayerId) => string

/** The cards of a loop as a list: "Polyraptor and Aether Flash", "A, B and C". */
export function loopCards(cards: readonly string[]): string {
  if (cards.length === 0) return 'A loop'
  if (cards.length === 1) return cards[0] ?? ''
  return `${cards.slice(0, -1).join(', ')} and ${cards.at(-1) ?? ''}`
}

function plural(count: number, word: string): string {
  return `${count} ${word}${count === 1 ? '' : 's'}`
}

/** What a step of a loop (one iteration, or all of them) changes, one phrase each: "+3 Polyraptor tokens for Ann". */
export function stepWords(step: LoopStep, seat: Namer): string[] {
  const tokens = (step.tokens ?? []).map(
    (made) => `+${plural(made.count, `${made.name} token`)} for ${seat(made.controller)}`,
  )
  const life = (step.life ?? []).map((change) => {
    const who = seat(change.player)
    const you = who === 'you'
    return change.change < 0
      ? `${who} ${you ? 'lose' : 'loses'} ${Math.abs(change.change)} life`
      : `${who} ${you ? 'gain' : 'gains'} ${change.change} life`
  })
  return [...tokens, ...life]
}

/** The log line of a loop shortcut: "Polyraptor loop repeated 100 times: +100 Polyraptor tokens for Ann". */
export function loopSummaryLine(summary: LoopSummary, seat: Namer): string {
  const who = summary.player === undefined ? '' : ` by ${seat(summary.player)}`
  const head = `${loopCards(summary.cards)} loop repeated ${plural(summary.iterations, 'time')}${who}`
  const changes = stepWords(summary.total, seat)
  return changes.length === 0 ? head : `${head}: ${changes.join(', ')}`
}

/** The log line and result detail of a draw by a loop that would never end (CR 104.4b). */
export function loopDrawLine(cards: readonly string[]): string {
  return `${loopCards(cards)} would loop forever with no one able to stop it: the game is a draw`
}

/** Why a number typed for the loop is refused, or null when it is fine. */
export function iterationsProblem(text: string, max: number): string | null {
  if (!/^\d+$/.test(text.trim())) return 'Enter a whole number'
  const value = Number(text.trim())
  if (value < 1) return 'At least 1'
  if (value > max) return `At most ${max.toLocaleString('en-US')}`
  return null
}
