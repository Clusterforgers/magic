/** The other questions of a spell the dev table can show: `?prompt=divide`, `divide-three`, `costs`, `costs-gift`, `x`, `way`, `sacrifice-cost` and `unknown-kind`. */
import type { PendingDecision } from '@/play/store'
import type { DecisionKind, ObjectId, OptionalCost, Target } from '@/protocol/generated'
import { SEAT_BASE } from '../prompt/costsText'
import { creaturesOf, opponents, ownSpell } from './effectPrompts'
import type { Fixture } from './fixture'

/** Builds the decision the viewer of a fixture faces. */
type DevPrompt = (fixture: Fixture) => PendingDecision

const object = (id: ObjectId): Target => ({ Object: { id, counter: 0 } })

/** A decision of the viewer's, as decision 1. */
const decision = ({ view }: Fixture, kind: DecisionKind): PendingDecision => ({
  id: 1,
  decision: { player: view.viewer, kind },
})

/** The viewer's spell divides 3 damage between an opponent and the first creature that opponent controls. */
const divide: DevPrompt = (fixture) => {
  const { view } = fixture
  const [foe] = opponents(view)
  const [creature] = foe === undefined ? [] : creaturesOf(view, foe)
  const among: Target[] = [
    ...(creature === undefined ? [] : [object(creature.id)]),
    ...(foe === undefined ? [] : [{ Player: foe } satisfies Target]),
  ]
  return decision(fixture, {
    DivideAmount: {
      source: ownSpell(view)?.id ?? 0,
      total: 3,
      among,
      text: 'Divide 3 damage among the targets.',
      can_cancel: true,
    },
  })
}

/** The same spell with six damage among three of the opponents' creatures. */
const divideThree: DevPrompt = (fixture) => {
  const { view } = fixture
  const among = opponents(view)
    .flatMap((seat) => creaturesOf(view, seat))
    .slice(0, 3)
    .map((creature) => object(creature.id))
  return decision(fixture, {
    DivideAmount: {
      source: ownSpell(view)?.id ?? 0,
      total: 6,
      among,
      text: 'Divide 6 damage among up to three target creatures.',
      can_cancel: true,
    },
  })
}

const KICKERS: OptionalCost[] = [
  { index: 0, label: 'Kicker {2}{R}' },
  { index: 1, label: 'Buyback {3}' },
]

/** The viewer's spell has a kicker and a buyback to pay for, and can be taken back. */
const costs: DevPrompt = (fixture) =>
  decision(fixture, {
    ChooseCosts: { source: ownSpell(fixture.view)?.id ?? 0, optional: KICKERS, can_cancel: true },
  })

/** A kicker and a gift that can be promised to each opponent; the engine accepts one gift. */
const costsGift: DevPrompt = (fixture) => {
  const gifts = opponents(fixture.view).map((seat): OptionalCost => ({
    index: SEAT_BASE + seat,
    label: `Gift a card to player ${seat + 1}`,
  }))
  return decision(fixture, {
    ChooseCosts: {
      source: ownSpell(fixture.view)?.id ?? 0,
      optional: [KICKERS[0] as OptionalCost, ...gifts],
      can_cancel: true,
    },
  })
}

/** The viewer's spell asks for its X, up to 5, and can be taken back. */
const x: DevPrompt = (fixture) =>
  decision(fixture, {
    ChooseNumber: {
      source: ownSpell(fixture.view)?.id ?? 0,
      min: 0,
      max: 5,
      text: 'Choose a value for X.',
      can_cancel: true,
    },
  })

/** The viewer's spell asks which of two ways to cast it, and can be taken back. */
const way: DevPrompt = (fixture) =>
  decision(fixture, {
    ChooseOption: {
      source: ownSpell(fixture.view)?.id ?? 0,
      options: [{ Label: 'Mana cost {1}{R}' }, { Label: 'Overload {4}{R}{R}' }],
      text: 'Choose how to cast it.',
      can_cancel: true,
    },
  })

/** An ability of the viewer's first creature asks for a creature to sacrifice as its cost, and can be taken back. */
const sacrificeCost: DevPrompt = (fixture) => {
  const own = creaturesOf(fixture.view, fixture.view.viewer)
  return decision(fixture, {
    ChooseCards: {
      source: own[0]?.id ?? 0,
      from: own.slice(0, 3).map((creature) => creature.id),
      min: 1,
      max: 1,
      text: 'Sacrifice a creature.',
      can_cancel: true,
    },
  })
}

/** A decision of a kind this client does not know, as a newer server could send: the generic prompt answers it. */
const unknownKind: DevPrompt = (fixture) =>
  decision(fixture, { Frobnicate: { source: ownSpell(fixture.view)?.id ?? 0 } } as unknown as DecisionKind)

/** The decisions by their `?prompt=` name; the viewer is not the active seat. */
export const ANNOUNCE_PROMPTS: Record<string, DevPrompt> = {
  divide,
  'divide-three': divideThree,
  costs,
  'costs-gift': costsGift,
  x,
  way,
  'sacrifice-cost': sacrificeCost,
  'unknown-kind': unknownKind,
}
