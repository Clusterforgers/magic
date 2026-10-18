/** Words for where the game is: the turn banner, who has priority, what the game waits for, and mulligan progress. */
import type { PlayState } from '@/play/store'
import type { DecisionKind, MulliganStatus, PlayerId, PriorityView, Stage, Step } from '@/protocol/generated'

type Names = (seat: PlayerId) => string

/** Each step as a player says it. */
export const STEP_LABEL: Record<Step, string> = {
  Untap: 'Untap',
  Upkeep: 'Upkeep',
  Draw: 'Draw',
  Main1: 'Precombat main',
  BeginCombat: 'Beginning of combat',
  DeclareAttackers: 'Declare attackers',
  DeclareBlockers: 'Declare blockers',
  FirstStrikeDamage: 'First-strike damage',
  CombatDamage: 'Combat damage',
  EndCombat: 'End of combat',
  Main2: 'Postcombat main',
  End: 'End step',
  Cleanup: 'Cleanup',
}

/** The name of a decision kind, which the wire spells as the one key of an object. */
export type DecisionName = DecisionKind extends infer Kind
  ? Kind extends unknown
    ? keyof Kind
    : never
  : never

/** What each decision is called in the interface. */
export const DECISION_LABEL: Record<DecisionName, string> = {
  Mulligan: 'Mulligan',
  BottomCards: 'Put cards on the bottom',
  Priority: 'Priority',
  PayMana: 'Pay mana',
  DeclareAttackers: 'Declare attackers',
  DeclareBlockers: 'Declare blockers',
  AssignDamage: 'Assign combat damage',
  Discard: 'Discard',
}

/** The name of a decision kind; a new kind stops compiling here until it is named. */
export function decisionName(kind: DecisionKind): DecisionName {
  if ('Mulligan' in kind) return 'Mulligan'
  if ('BottomCards' in kind) return 'BottomCards'
  if ('Priority' in kind) return 'Priority'
  if ('PayMana' in kind) return 'PayMana'
  if ('DeclareAttackers' in kind) return 'DeclareAttackers'
  if ('DeclareBlockers' in kind) return 'DeclareBlockers'
  if ('AssignDamage' in kind) return 'AssignDamage'
  void (kind satisfies { Discard: unknown })
  return 'Discard'
}

/** The inputs of the turn banner. */
export interface BannerInput {
  stage: Stage
  turn: number
  active: PlayerId
  step: Step
}

/** "Turn 5 · Alice · Declare attackers", or what the game is doing before and after its turns. */
export function bannerText(input: BannerInput, name: Names): string {
  switch (input.stage) {
    case 'PreGame':
      return 'Setting up the game'
    case 'Mulligan':
      return 'Opening hands'
    case 'Finished':
      return 'The game is over'
    case 'Turns':
      return `Turn ${input.turn} · ${name(input.active)} · ${STEP_LABEL[input.step]}`
  }
}

/** Who has priority and how many have passed it on. */
export function priorityText(priority: PriorityView | null, stage: Stage, name: Names): string {
  if (priority === null) return stage === 'Turns' ? 'Nobody has priority' : ''
  const passed = priority.passed.length
  return `${name(priority.holder)} has priority${passed === 0 ? '' : ` · ${passed} passed`}`
}

/** Opening-hand progress of one seat. */
export function describeMulligan(status: MulliganStatus, mulligans: number): string {
  const taken = mulligans === 0 ? '' : ` · ${mulligans} ${mulligans === 1 ? 'mulligan' : 'mulligans'}`
  if (typeof status !== 'string') {
    return `Putting ${status.Bottoming} ${status.Bottoming === 1 ? 'card' : 'cards'} on the bottom${taken}`
  }
  switch (status) {
    case 'NeedsHand':
      return `Waiting for a new hand${taken}`
    case 'Deciding':
      return `Deciding whether to keep${taken}`
    case 'Mulliganing':
      return `Mulligan taken, waiting for the table${taken}`
    case 'Kept':
      return `Kept their hand${taken}`
  }
}

/** What the game is waiting for: this seat's own decision, other seats, or nothing in particular. */
export type Waiting =
  { kind: 'you'; label: string } | { kind: 'others'; seats: PlayerId[] } | { kind: 'idle'; label: string }

/** What the game waits for now, read from the pending decision and the table. */
export function waitingOn(state: PlayState): Waiting {
  const { pending, view, seat } = state
  if (pending !== null) return { kind: 'you', label: DECISION_LABEL[decisionName(pending.decision.kind)] }
  if (view === null) return { kind: 'idle', label: 'Waiting for the game' }
  if (view.stage === 'Finished' || view.result !== null) return { kind: 'idle', label: 'The game is over' }
  const others = (seats: PlayerId[]): Waiting =>
    seats.length > 0 ? { kind: 'others', seats } : { kind: 'idle', label: 'Waiting for the game' }
  if (view.stage === 'Mulligan') {
    const deciding = view.players.filter(
      (player) =>
        player.lost === null && (player.mulligan === 'Deciding' || typeof player.mulligan !== 'string'),
    )
    return others(deciding.map((player) => player.seat).filter((who) => who !== seat))
  }
  if (view.priority !== null) return others(view.priority.holder === seat ? [] : [view.priority.holder])
  if (view.turn.step === 'DeclareAttackers' && !view.combat.attackers_declared) {
    return others(view.turn.active === seat ? [] : [view.turn.active])
  }
  return { kind: 'idle', label: 'Waiting for the game' }
}

/** The line under the banner: "Your decision: Priority", "Waiting for Bob". */
export function waitingText(waiting: Waiting, name: Names): string {
  switch (waiting.kind) {
    case 'you':
      return `Your decision: ${waiting.label}`
    case 'idle':
      return waiting.label
    case 'others': {
      if (waiting.seats.length > 3) return `Waiting for ${waiting.seats.length} players`
      const names = waiting.seats.map(name)
      const last = names.pop()
      return `Waiting for ${names.length === 0 ? last : `${names.join(', ')} and ${last}`}`
    }
  }
}
