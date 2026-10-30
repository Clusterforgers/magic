/**
 * Which lines the overlay draws, as data: the declared combat (attacks muted, blocks strong, coloured by the
 * forecast's outcome) and the choices a prompt has not sent yet (dashed). Pure; the DOM comes in later, by anchor.
 */
import type { AttackTarget, CombatForecast, CombatView, ObjectId, PlayerId } from '@/protocol/generated'
import { blockOutcome } from './outcome'
import type { Outcome } from './outcome'
import type { PendingCombat } from './pending'

/** A place a line starts or ends: a card, or a seat's panel. */
export type Anchor =
  | {
      kind: 'card'
      id: ObjectId
      /** Who controls the card, so that a card that is not drawn can stand in as its controller's seat. */
      seat?: PlayerId
    }
  | { kind: 'seat'; seat: PlayerId }
  /** The chip of an item on the stack. */
  | { kind: 'stack'; id: ObjectId }

/**
 * An attack goes from an attacker to what it attacks; a block from a blocker to the attacker it blocks; a target
 * from the chip of a stack item to what it aims at (`stackArrows.ts`); an attach from the old host of an attachment
 * to its new one (`attachLines.ts`).
 */
export type LineKind = 'attack' | 'block' | 'target' | 'attach'

/** One line of the overlay. */
export interface LineSpec {
  /** Stable while the line stays the same, so its element lives on while its path moves. */
  key: string
  kind: LineKind
  /** Chosen in a prompt and not yet sent. */
  pending: boolean
  from: Anchor
  to: Anchor
  /** What the forecast says happens to the two creatures of a declared block; absent for the rest. */
  outcome?: Outcome
  /** For a target line: the stack item belongs to the viewer. */
  mine?: boolean
}

/** What the plan is drawn from. */
export interface PlanInput {
  combat: CombatView | null
  /** The forecast to draw, or null for none. */
  forecast: CombatForecast | null
  pending: PendingCombat
  /** Who controls a card; unknown cards give undefined. */
  controllerOf: (id: ObjectId) => PlayerId | undefined
}

/** A key that is the same for the same anchor, to look its box up by. */
export const anchorKey = (anchor: Anchor): string =>
  anchor.kind === 'seat' ? `seat:${anchor.seat}` : `${anchor.kind}:${anchor.id}`

/** The lines to draw: attacks first, then blocks, then the unsent choices on top. */
export function planLines({ combat, forecast, pending, controllerOf }: PlanInput): LineSpec[] {
  const card = (id: ObjectId): Anchor => {
    const seat = controllerOf(id)
    return seat === undefined ? { kind: 'card', id } : { kind: 'card', id, seat }
  }
  const target = (to: AttackTarget): Anchor =>
    'Player' in to ? { kind: 'seat', seat: to.Player } : card(to.Planeswalker)

  const attacks: LineSpec[] = []
  const blocks: LineSpec[] = []
  if (combat?.attackers_declared === true) {
    for (const attacker of combat.attackers) {
      attacks.push({
        key: `attack:${attacker.object}`,
        kind: 'attack',
        pending: false,
        from: card(attacker.object),
        to: target(attacker.target),
      })
      for (const blocker of attacker.blockers) {
        const outcome = forecast === null ? undefined : blockOutcome(forecast, attacker.object, blocker)
        blocks.push({
          key: `block:${blocker}`,
          kind: 'block',
          pending: false,
          from: card(blocker),
          to: card(attacker.object),
          ...(outcome === undefined ? {} : { outcome }),
        })
      }
    }
  }
  const chosen: LineSpec[] = [
    ...[...pending.attacks].map(([attacker, to]): LineSpec => ({
      key: `pending-attack:${attacker}`,
      kind: 'attack',
      pending: true,
      from: card(attacker),
      to: target(to),
    })),
    ...[...pending.blocks].map(([blocker, attacker]): LineSpec => ({
      key: `pending-block:${blocker}`,
      kind: 'block',
      pending: true,
      from: card(blocker),
      to: card(attacker),
    })),
  ]
  return [...attacks, ...blocks, ...chosen]
}

/** Every distinct anchor the lines touch, in the order they first appear. */
export function anchorsOf(lines: readonly LineSpec[]): Anchor[] {
  const found = new Map<string, Anchor>()
  for (const line of lines) {
    for (const anchor of [line.from, line.to]) {
      if (!found.has(anchorKey(anchor))) found.set(anchorKey(anchor), anchor)
    }
  }
  return [...found.values()]
}
