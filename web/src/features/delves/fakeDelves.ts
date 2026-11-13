/** Runs and content for the Delves tests: a run in any state and a client that answers from memory. */
import type { DelvesApi } from '@/net/delves'
import type { DelveAction, DelveContent, DelveView, Hero, Run } from '@/protocol/generated'

export const CONTENT: DelveContent = {
  theme: 'The Warrens',
  text: 'Tunnels under the old mines.',
  cards: [
    {
      id: 'skrix',
      name: 'Skrix, the Rat Queen',
      kind: 'Boss',
      type_line: 'Boss — Rat Queen',
      text: 'Rats.',
      art: 'skrix',
      role: '',
      price: 0,
    },
    {
      id: 'nest',
      name: 'Phase II — The Nest Stirs',
      kind: 'Phase',
      type_line: 'Boss Phase',
      text: 'More rats.',
      art: 'nest',
      role: '',
      price: 0,
    },
    {
      id: 'idol',
      name: 'Ember Idol',
      kind: 'Relic',
      type_line: 'Relic',
      text: 'Creatures you control get +1/+0.',
      art: 'idol',
      role: 'Damage',
      price: 70,
    },
    {
      id: 'pact',
      name: 'Wildfire Pact',
      kind: 'Relic',
      type_line: 'Relic — Risky',
      text: 'All red spells cost {2} less.',
      art: 'pact',
      role: 'Risky',
      price: 0,
    },
  ],
  bosses: [
    {
      id: 'skrix_nest',
      name: 'Skrix, the Rat Queen',
      colors: 'B',
      line: 'My children are hungry.',
      card: 'skrix',
      phases: [['nest', 66]],
      life: [30, 35],
    },
    { id: 'grimgut', name: 'Grimgut', colors: 'BR', line: 'Kneel.', card: null, phases: [], life: [36, 40] },
  ],
  events: [
    {
      id: 'toll',
      title: 'The Goblin Toll',
      text: 'A goblin blocks a bridge.',
      options: ['Pay 25 gold', 'Turn back'],
    },
  ],
}

export function hero(over: Partial<Hero> = {}): Hero {
  return {
    name: 'Ana',
    user: 'u0',
    bot: null,
    deck: null,
    life: 40,
    gold: 0,
    relics: [],
    cards: [],
    offers: [],
    ...over,
  }
}

export function run(over: Partial<Run> = {}): Run {
  return {
    heat: 0,
    theme: 'warrens',
    map: {
      nodes: [
        { id: 0, row: 0, lane: 0, kind: 'boss', boss: 'skrix_nest', next: [1, 2] },
        { id: 1, row: 1, lane: 0, kind: 'shop', next: [3] },
        { id: 2, row: 1, lane: 1, kind: 'event', next: [3] },
        { id: 3, row: 2, lane: 0, kind: 'boss', boss: 'grimgut', next: [] },
      ],
    },
    heroes: [hero()],
    at: null,
    cleared: [],
    votes: {},
    hearts: 3,
    status: 'Lobby',
    log: ['Ana joined the party.'],
    ...over,
  }
}

export function delveView(over: Partial<DelveView> = {}, runOver: Partial<Run> = {}): DelveView {
  return { id: 'd1', code: 'CODE1', seed: 'SEED', host: true, me: 0, run: run(runOver), ...over }
}

/** A client that answers every read with `view` and records the actions. */
export function fakeDelves(view: DelveView): DelvesApi & { actions: DelveAction[] } {
  const actions: DelveAction[] = []
  return {
    actions,
    list: async () => ({ delves: [] }),
    create: async () => view,
    join: async () => view,
    get: async () => view,
    act: async (_id, action) => {
      actions.push(action)
      return view
    },
    content: async () => CONTENT,
  }
}
