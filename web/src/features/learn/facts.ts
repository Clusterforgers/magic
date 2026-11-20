import type { PlayState } from '@/play/store'
import type { CardType, ObjectId, Step, Stop, StopSide } from '@/protocol/generated'
import { decisionName } from '@/features/table/turnText'
import type { DecisionName } from '@/features/table/turnText'
import type { AnchorId } from './anchors'

/** A card by name and id. */
export interface Named {
  name: string
  id: ObjectId
}

/**
 * The game as a lesson reads it: the same fields the server's tutorial test writes into a trace
 * (`crates/mtg-server/tests/it/tutorial.rs`), so a lesson's steps are tested against what the table really sends.
 */
export interface LessonFacts {
  /** The turn number, 0 before the first view. */
  turn: number
  /** Whether it is the learner's turn. */
  mine: boolean
  /** The step of the turn, null before the first view. */
  step: Step | null
  /** The kind of the decision waiting for the learner, null while none waits. */
  decision: DecisionName | 'Unknown' | null
  hand: readonly Named[]
  /** The learner's lands and creatures on the battlefield, and the opponent's creatures. */
  lands: readonly Named[]
  creatures: readonly Named[]
  theirCreatures: readonly Named[]
  /** The learner's command zone. */
  command: readonly Named[]
  /** The graveyards the learner and the opponent own. */
  graveyard: readonly Named[]
  theirGraveyard: readonly Named[]
  /** The stack, bottom first, and whether the learner controls each. */
  stack: readonly (Named & { mine: boolean })[]
  /** The learner's creatures that attack and that block, and whether a creature attacks the learner. */
  attacking: number
  blocking: number
  attacked: boolean
  /** Both life totals, null before the first view. */
  life: number | null
  theirLife: number | null
  /** The stops the server holds for the learner. */
  stops: readonly { side: StopSide; step: Step }[]
  /** Whether the game is over, and whether the learner won it. */
  over: boolean
  won: boolean
}

/** What the page shows beside the game: the anchors drawn now. */
export interface PageFacts {
  shown: ReadonlySet<AnchorId>
  /** The manual actions the learner took, from the log. */
  byHand?: number
}

/** The facts a step's `done` reads. */
export type Facts = LessonFacts & { page: PageFacts }

/** A page that shows nothing extra. */
export const NO_PAGE: PageFacts = { shown: new Set() }

/** The facts of `state` as the learner's seat sees it. */
export function lessonFacts(state: PlayState): LessonFacts {
  const view = state.view
  const seat = state.seat
  const result = state.result
  const won =
    seat !== null && result !== null && result !== 'Draw' && 'Winner' in result && result.Winner === seat
  const empty: LessonFacts = {
    turn: 0,
    mine: false,
    step: null,
    decision: null,
    hand: [],
    lands: [],
    creatures: [],
    theirCreatures: [],
    command: [],
    graveyard: [],
    theirGraveyard: [],
    stack: [],
    attacking: 0,
    blocking: 0,
    attacked: false,
    life: null,
    theirLife: null,
    stops: state.stops.map(({ side, step }: Stop) => ({ side, step })),
    over: result !== null,
    won,
  }
  if (view === null || seat === null) return empty
  const named = (id: ObjectId): Named => ({ id, name: state.objectsById[id]?.name ?? '' })
  const objects = view.objects
  const onField = (type: CardType, mine: boolean): Named[] =>
    objects
      .filter(
        (object) =>
          object.zone === 'Battlefield' &&
          (object.controller === seat) === mine &&
          object.types.includes(type),
      )
      .map((object) => named(object.id))
  const inZone = (zone: 'Hand' | 'Command' | 'Graveyard', owner: number): Named[] =>
    objects
      .filter((object) => typeof object.zone === 'object' && zone in object.zone && object.owner === owner)
      .map((object) => named(object.id))
  const other = view.players.find((player) => player.seat !== seat)?.seat ?? -1
  const own = new Set(objects.filter((object) => object.controller === seat).map((object) => object.id))
  const attackers = view.combat.attackers
  return {
    ...empty,
    turn: view.turn.number,
    mine: view.turn.active === seat,
    step: view.turn.step,
    decision: state.pending === null ? null : decisionName(state.pending.decision.kind),
    hand: inZone('Hand', seat),
    lands: onField('Land', true),
    creatures: onField('Creature', true),
    theirCreatures: onField('Creature', false),
    command: inZone('Command', seat),
    graveyard: inZone('Graveyard', seat),
    theirGraveyard: inZone('Graveyard', other),
    stack: (view.stack.cards ?? []).map((id) => ({
      ...named(id),
      mine: state.objectsById[id]?.controller === seat,
    })),
    attacking: attackers.filter((attacker) => own.has(attacker.object)).length,
    blocking: attackers.reduce(
      (count, attacker) => count + attacker.blockers.filter((blocker) => own.has(blocker)).length,
      0,
    ),
    attacked: attackers.some((attacker) => !own.has(attacker.object)),
    life: view.players.find((player) => player.seat === seat)?.life ?? null,
    theirLife: view.players.find((player) => player.seat === other)?.life ?? null,
  }
}

/** Whether `cards` holds one called `name`. */
export const has = (cards: readonly Named[], name: string): boolean =>
  cards.some((card) => card.name === name)

/** How many of `cards` are called `name`. */
export const count = (cards: readonly Named[], name: string): number =>
  cards.filter((card) => card.name === name).length
