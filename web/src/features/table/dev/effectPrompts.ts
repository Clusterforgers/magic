/** The decisions of spells and abilities the dev table can show: `?prompt=targets`, `targets-two`, `targets-trigger`, `order` and their clocks. */
import type { PendingDecision } from '@/play/store'
import type { ObjectId, ObjectView, PlayerId, StateView, Target, TriggerEntry } from '@/protocol/generated'
import type { Fixture } from './fixture'

/** Builds the decision the viewer of a fixture faces. */
type DevPrompt = (fixture: Fixture) => PendingDecision

const object = (id: ObjectId): Target => ({ Object: { id, counter: 0 } })

/** The viewer's own spell on the stack of the fixture, which asks for its targets. */
const ownSpell = (view: StateView): ObjectView | undefined =>
  view.objects.find((o) => o.zone === 'Stack' && o.controller === view.viewer)

/** The creatures `seat` controls on the battlefield, in the order of the table. */
const creaturesOf = (view: StateView, seat: PlayerId): ObjectView[] =>
  view.objects.filter(
    (o) => o.zone === 'Battlefield' && o.controller === seat && o.types.includes('Creature'),
  )

/** Every seat but the viewer's that is still in. */
const opponents = (view: StateView): PlayerId[] =>
  view.players.filter((p) => p.seat !== view.viewer && p.lost === null).map((p) => p.seat)

/** The viewer's spell asks for one target: an opponent or a creature one of them controls. */
const targets: DevPrompt = ({ view }) => {
  const foes = opponents(view)
  const creatures = foes.flatMap((seat) => creaturesOf(view, seat).slice(0, 3))
  return {
    id: 1,
    decision: {
      player: view.viewer,
      kind: {
        ChooseTargets: {
          source: ownSpell(view)?.id ?? 0,
          spec: 0,
          legal: [...foes.map((seat): Target => ({ Player: seat })), ...creatures.map((o) => object(o.id))],
          min: 1,
          max: 1,
          can_cancel: true,
          wording: 'target creature or player an opponent controls',
        },
      },
    },
  }
}

/** The same spell asks for up to two creatures, some of them the viewer's own. */
const targetsTwo: DevPrompt = ({ view }) => {
  const mine = creaturesOf(view, view.viewer).slice(0, 2)
  const theirs = opponents(view).flatMap((seat) => creaturesOf(view, seat).slice(0, 2))
  return {
    id: 1,
    decision: {
      player: view.viewer,
      kind: {
        ChooseTargets: {
          source: ownSpell(view)?.id ?? 0,
          spec: 0,
          legal: [...mine, ...theirs].map((o) => object(o.id)),
          min: 0,
          max: 2,
          can_cancel: true,
          wording: 'up to two target creatures',
        },
      },
    },
  }
}

/** A triggered ability of one of the viewer's creatures asks for a target; it cannot be backed out of. */
const targetsTrigger: DevPrompt = ({ view }) => {
  const [source] = creaturesOf(view, view.viewer)
  const theirs = opponents(view).flatMap((seat) => creaturesOf(view, seat).slice(0, 3))
  return {
    id: 1,
    decision: {
      player: view.viewer,
      kind: {
        ChooseTargets: {
          source: source?.id ?? 0,
          spec: 0,
          legal: theirs.map((o) => object(o.id)),
          min: 1,
          max: 1,
          can_cancel: false,
          wording: 'target creature an opponent controls',
          triggered: true,
        },
      },
    },
  }
}

const WHENS = [
  'When it enters the battlefield',
  'Whenever it attacks',
  'When it dies',
  'At the beginning of your upkeep',
]

/** Up to four triggers of the viewer's creatures wait to go on the stack together. */
const order: DevPrompt = ({ view }) => {
  const triggers: TriggerEntry[] = creaturesOf(view, view.viewer)
    .slice(0, WHENS.length)
    .map((o, index) => ({ source: { id: o.id, counter: 0 }, ability: 0, when: WHENS[index] ?? '' }))
  return { id: 1, decision: { player: view.viewer, kind: { OrderTriggers: { triggers } } } }
}

/** The decisions by their `?prompt=` name; the viewer is not the active seat, so these run on a response clock. */
export const EFFECT_PROMPTS: Record<string, DevPrompt> = {
  targets,
  'targets-two': targetsTwo,
  'targets-trigger': targetsTrigger,
  'targets-clock': (fixture) => ({ ...targets(fixture), deadlineMs: 10_000 }),
  order,
  'order-clock': (fixture) => ({ ...order(fixture), deadlineMs: 10_000 }),
}
