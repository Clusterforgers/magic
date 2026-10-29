/** The decisions of spells and abilities the dev table can show: `?prompt=targets`, `targets-two`, `targets-trigger`, `order`, `yesno`, `option`, `option-players`, `mode`, `mode-two`, `mode-trigger`, `cards`, `cards-battlefield`, `cards-many`, `order-cards` and their clocks. */
import type { PendingDecision } from '@/play/store'
import type {
  ChoiceOption,
  Color,
  ObjectId,
  ObjectView,
  PlayerId,
  StateView,
  Target,
  TriggerEntry,
} from '@/protocol/generated'
import { CREATURES, INSTANTS, LANDS } from './cards'
import type { Template } from './cards'
import type { Fixture } from './fixture'

/** Builds the decision the viewer of a fixture faces. */
type DevPrompt = (fixture: Fixture) => PendingDecision

/** Changes a fixture into the moment a decision belongs to. */
type DevMoment = (fixture: Fixture) => Fixture

const object = (id: ObjectId): Target => ({ Object: { id, counter: 0 } })

/** The viewer's own spell on the stack of the fixture, which asks for its targets. */
export const ownSpell = (view: StateView): ObjectView | undefined =>
  view.objects.find((o) => o.zone === 'Stack' && o.controller === view.viewer)

/** The creatures `seat` controls on the battlefield, in the order of the table. */
export const creaturesOf = (view: StateView, seat: PlayerId): ObjectView[] =>
  view.objects.filter(
    (o) => o.zone === 'Battlefield' && o.controller === seat && o.types.includes('Creature'),
  )

/** Every seat but the viewer's that is still in. */
export const opponents = (view: StateView): PlayerId[] =>
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

/** The first creature of the viewer's, whose ability asks. */
const askerOf = (view: StateView): ObjectId => creaturesOf(view, view.viewer)[0]?.id ?? 0

/** A "may" of one of the viewer's creatures. */
const yesNo: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: { YesNo: { source: askerOf(view), text: 'Draw a card?' } },
  },
})

const COLORS: Color[] = ['White', 'Blue', 'Black', 'Red', 'Green']

/** One of the viewer's creatures asks for a color. */
const option: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      ChooseOption: {
        source: askerOf(view),
        options: COLORS.map((color): ChoiceOption => ({ Color: color })),
        text: 'Choose a color.',
      },
    },
  },
})

/** The same kind of question about the players still in the game, the viewer's own seat among them. */
const optionPlayers: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      ChooseOption: {
        source: askerOf(view),
        options: view.players.filter((p) => p.lost === null).map((p): ChoiceOption => ({ Player: p.seat })),
        text: 'Choose a player to attack first.',
      },
    },
  },
})

const CHARM = [
  'Deal 2 damage to target creature unless its controller discards a card',
  'Counter target noncreature spell unless its controller pays {2}',
  'Draw two cards, then discard two cards',
]

const BLESSING = ['Draw a card', 'Create a 1/1 Spirit token', 'You gain 3 life', 'Each opponent loses 2 life']

const labelled = (labels: readonly string[]) => labels.map((label, index) => ({ index, label }))

/** The viewer's spell asks for one of three modes and can still be taken back. */
const mode: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      ChooseMode: {
        source: ownSpell(view)?.id ?? 0,
        modes: labelled(CHARM),
        min: 1,
        max: 1,
        repeat: false,
        can_cancel: true,
      },
    },
  },
})

/** The same spell asks for two of four modes. */
const modeTwo: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      ChooseMode: {
        source: ownSpell(view)?.id ?? 0,
        modes: labelled(BLESSING),
        min: 2,
        max: 2,
        repeat: false,
        can_cancel: true,
      },
    },
  },
})

/** A triggered ability of one of the viewer's creatures chooses a mode as it goes on the stack; it cannot be backed out of. */
const modeTrigger: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      ChooseMode: {
        source: askerOf(view),
        modes: labelled(BLESSING.slice(0, 3)),
        min: 1,
        max: 1,
        repeat: false,
        can_cancel: false,
      },
    },
  },
})

/** The viewer's hand. */
const handOf = (view: StateView): ObjectId[] => view.players[view.viewer]?.hand.cards ?? []

/** An opponent's creature, whose ability makes the viewer choose. */
const foeOf = (view: StateView): ObjectId =>
  opponents(view).flatMap((seat) => creaturesOf(view, seat))[0]?.id ?? askerOf(view)

/** An opponent's spell makes the viewer discard two cards of their hand. */
const cards: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      ChooseCards: { source: foeOf(view), from: handOf(view), min: 2, max: 2, text: 'Discard two cards.' },
    },
  },
})

/** An opponent's creature makes the viewer sacrifice one of four of their creatures, which are on the table. */
const cardsBattlefield: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      ChooseCards: {
        source: foeOf(view),
        from: creaturesOf(view, view.viewer)
          .slice(0, 4)
          .map((o) => o.id),
        min: 1,
        max: 1,
        text: 'Sacrifice a creature or planeswalker.',
      },
    },
  },
})

/** The search the viewer makes: up to two of the forty cards of their library. */
const cardsMany: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      ChooseCards: {
        source: askerOf(view),
        from: view.objects
          .filter((o) => typeof o.zone === 'object' && 'Library' in o.zone && o.zone.Library === view.viewer)
          .map((o) => o.id),
        min: 0,
        max: 2,
        text: 'Search your library for up to two cards.',
      },
    },
  },
})

const SEARCHED: readonly Template[] = [...LANDS, ...CREATURES, ...INSTANTS]
const SEARCH_COUNT = 40

/** The viewer's library as a search shows it: forty cards, several of them twice. */
const searchMoment: DevMoment = (fixture) => {
  const { view } = fixture
  const first = Math.max(...view.objects.map((o) => o.id)) + 1
  const found = Array.from({ length: SEARCH_COUNT }, (_, index): ObjectView => {
    const template = SEARCHED[(index * 7) % SEARCHED.length] ?? LANDS[0]
    return {
      id: first + index,
      owner: view.viewer,
      controller: view.viewer,
      zone: { Library: view.viewer },
      name: template.name,
      mana_cost: template.cost,
      supertypes: template.supertypes ?? [],
      types: template.types,
      subtypes: template.subtypes ?? [],
      colors: template.colors,
      power: template.power ?? null,
      toughness: template.toughness ?? null,
      abilities: template.produces === undefined ? [] : [{ Mana: { produces: template.produces } }],
      permanent: null,
    }
  })
  return { ...fixture, view: { ...view, objects: [...view.objects, ...found] } }
}

/** The viewer puts four cards of their hand on the bottom of their library in any order. */
const orderCards: DevPrompt = ({ view }) => ({
  id: 1,
  decision: {
    player: view.viewer,
    kind: {
      OrderCards: {
        source: askerOf(view),
        cards: handOf(view).slice(0, 4),
        text: 'Put the cards on the bottom of your library in any order.',
      },
    },
  },
})

/** The moments the dev table needs for some of the decisions above, by `?prompt=` name. */
export const EFFECT_MOMENTS: Record<string, DevMoment> = {
  'cards-many': searchMoment,
  'cards-many-clock': searchMoment,
}

/** The decisions by their `?prompt=` name; the viewer is not the active seat, so these run on a response clock. */
export const EFFECT_PROMPTS: Record<string, DevPrompt> = {
  targets,
  'targets-two': targetsTwo,
  'targets-trigger': targetsTrigger,
  'targets-clock': (fixture) => ({ ...targets(fixture), deadlineMs: 10_000 }),
  order,
  'order-clock': (fixture) => ({ ...order(fixture), deadlineMs: 10_000 }),
  yesno: yesNo,
  'yesno-clock': (fixture) => ({ ...yesNo(fixture), deadlineMs: 10_000 }),
  option,
  'option-players': optionPlayers,
  'option-clock': (fixture) => ({ ...option(fixture), deadlineMs: 10_000 }),
  mode,
  'mode-two': modeTwo,
  'mode-trigger': modeTrigger,
  'mode-clock': (fixture) => ({ ...mode(fixture), deadlineMs: 10_000 }),
  cards,
  'cards-clock': (fixture) => ({ ...cards(fixture), deadlineMs: 10_000 }),
  'cards-battlefield': cardsBattlefield,
  'cards-battlefield-clock': (fixture) => ({ ...cardsBattlefield(fixture), deadlineMs: 10_000 }),
  'cards-many': cardsMany,
  'cards-many-clock': (fixture) => ({ ...cardsMany(fixture), deadlineMs: 10_000 }),
  'order-cards': orderCards,
  'order-cards-clock': (fixture) => ({ ...orderCards(fixture), deadlineMs: 10_000 }),
}
