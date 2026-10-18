/** Decisions the dev table can show: `/dev/table?prompt=<name>`. Add a line to `DEV_PROMPTS` for each new prompt. */
import type { PendingDecision } from '@/play/store'
import type {
  DecisionKind,
  ManaOption,
  ObjectId,
  ObjectView,
  PayOptions,
  PriorityOptions,
} from '@/protocol/generated'
import type { Fixture } from './fixture'

/** Builds the decision the viewer of a fixture faces. */
export type DevPrompt = (fixture: Fixture) => PendingDecision

const isLand = (object: ObjectView): boolean => object.types.includes('Land')

/**
 * Priority over the fixture's stack: the hand's lands may be played, all but the last spell of the hand may be cast
 * and every untapped permanent that makes mana may be tapped. The first non-land mana source gets an invented second
 * ability, since the fixture has none with two, so the choice between abilities can be seen.
 */
const priority: DevPrompt = ({ view }) => {
  const viewer = view.viewer
  const hand = view.objects.filter((object) => view.players[viewer]?.hand.cards?.includes(object.id))
  const spells = hand.filter((object) => !isLand(object))
  const sources = view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === viewer &&
      object.permanent?.tapped === false &&
      object.abilities.length > 0,
  )
  const mana_abilities = sources.flatMap((object): ManaOption[] => {
    const own = object.abilities.flatMap((ability, index): ManaOption[] =>
      'Mana' in ability
        ? [{ source: { object: object.id, ability: index }, produces: ability.Mana.produces }]
        : [],
    )
    const [first] = own
    const invented = !isLand(object) && object === sources.find((source) => !isLand(source))
    return invented && first !== undefined
      ? [...own, { source: { object: object.id, ability: first.source.ability + 1 }, produces: 'Red' }]
      : own
  })
  const options: PriorityOptions = {
    playable_lands: hand.filter(isLand).map((object) => object.id),
    castable: spells.slice(0, -1).map((object) => object.id),
    mana_abilities,
    can_pass: true,
  }
  return { id: 1, decision: { player: viewer, kind: { Priority: options } } }
}

/**
 * Paying for Selesnya Evangel ({2}{W}{G}) from the hand, with the viewer's floating mana (two red, a green, a
 * colourless), every untapped land and mana rock, and one invented second ability (red) on the first rock so the
 * choice between abilities can be seen. Costs have no hybrid or X, so there is one cost to show.
 */
const pay: DevPrompt = ({ view }) => {
  const viewer = view.viewer
  const spell = view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === viewer)
  const sources = view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === viewer &&
      object.permanent?.tapped === false &&
      object.abilities.length > 0,
  )
  const rock = sources.find((object) => !object.types.includes('Land'))
  const options: PayOptions = {
    spell: spell?.id ?? 0,
    cost: spell?.mana_cost ?? { generic: 2, colorless: 0, colored: [1, 0, 0, 0, 1] },
    pool: view.players[viewer]?.mana ?? { counts: [0, 0, 0, 0, 0, 0] },
    sources: sources.flatMap((object): ManaOption[] =>
      object.abilities.flatMap((ability, index): ManaOption[] =>
        'Mana' in ability
          ? [{ source: { object: object.id, ability: index }, produces: ability.Mana.produces }]
          : [],
      ),
    ),
    auto_available: true,
  }
  if (rock !== undefined) {
    options.sources.push({ source: { object: rock.id, ability: rock.abilities.length }, produces: 'Red' })
  }
  return { id: 1, decision: { player: viewer, kind: { PayMana: options } } }
}

/** A cost the viewer cannot pay (two blue, with three sources and none of them blue): no automatic payment. */
const payShort: DevPrompt = (fixture) => {
  const { id, decision } = pay(fixture)
  if (!('PayMana' in decision.kind)) return { id, decision }
  const { PayMana: options } = decision.kind
  const short: PayOptions = {
    ...options,
    cost: { generic: 3, colorless: 0, colored: [0, 2, 0, 0, 0] },
    pool: { counts: [0, 0, 0, 0, 0, 0] },
    sources: options.sources.filter((option) => option.produces !== 'Blue').slice(0, 3),
    auto_available: false,
  }
  return { id, decision: { ...decision, kind: { PayMana: short } } }
}

/** A decision about the viewer's hand, such as an opening hand to judge or cards to discard. */
const aboutHand =
  (kind: (hand: ObjectId[]) => DecisionKind): DevPrompt =>
  ({ view }) => ({
    id: 1,
    decision: { player: view.viewer, kind: kind(view.players[view.viewer]?.hand.cards ?? []) },
  })

/** The prompts by their `?prompt=` name. */
export const DEV_PROMPTS: Record<string, DevPrompt> = {
  priority,
  mulligan: aboutHand((hand) => ({ Mulligan: { hand, mulligans_taken: 1, can_mulligan: true } })),
  bottom: aboutHand((hand) => ({ BottomCards: { hand, count: 2 } })),
  discard: aboutHand((hand) => ({ Discard: { hand: hand.slice(0, -1), count: 2 } })),
  pay,
  'pay-short': payShort,
}
