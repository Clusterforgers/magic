/** Runs and content for the Delves tests: a run in any state and a client that answers from memory. */
import type { DelvesApi } from '@/net/delves'
import type { DelveAction, DelveContent, DelveView, Hero, Needs, Run } from '@/protocol/generated'

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
      rarity: 'Common',
      turn: 0,
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
      rarity: 'Common',
      turn: 0,
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
      rarity: 'Rare',
      turn: 0,
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
      rarity: 'Mythic',
      turn: 0,
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
      art: { card: 'Ashcoat of the Shadow Swarm', oracle_id: '9cc69ea5-42a2-4306-ac6d-cff5adb20bcb' },
    },
    {
      id: 'grimgut',
      name: 'Grimgut',
      colors: 'BR',
      line: 'Kneel.',
      card: null,
      phases: [],
      life: [36, 40],
      art: { card: 'Krenko, Tin Street Kingpin', oracle_id: 'e8065e1d-e937-4b56-8011-78f0d07328a0' },
    },
  ],
  events: [
    {
      id: 'toll',
      title: 'The Goblin Toll',
      text: 'A goblin blocks a bridge.',
      options: [
        {
          label: 'Pay the toll',
          outcomes: [{ Gold: -25 }, 'CardPick'],
          needs: needs({ gold: 25 }),
          summary: '−25 gold, a card for your Satchel',
          cost: '25 gold',
          reward: 'a card for your Satchel',
        },
        {
          label: 'Push past',
          outcomes: [{ Life: -8 }, { Gold: 40 }],
          needs: needs({ life: 8 }),
          summary: '−8 life, +40 gold',
          cost: '8 life',
          reward: '40 gold',
        },
        {
          label: 'Reach into the sack',
          outcomes: [{ Gamble: { odds: 50, win: ['Relic'], lose: [{ Life: -10 }] } }],
          needs: needs(),
          summary: '50%: a relic · 50%: −10 life',
          cost: 'nothing',
          reward: '50%: a relic · 50%: −10 life',
        },
        {
          label: 'Turn back',
          outcomes: [],
          needs: needs(),
          summary: 'nothing',
          cost: 'nothing',
          reward: 'nothing',
        },
      ],
    },
    {
      id: 'bargain',
      title: 'The Hollow Bargain',
      text: 'A voice offers power for a price.',
      options: [
        {
          label: 'Bleed for it',
          outcomes: [{ MaxLife: -5 }, { RelicOf: 'Mythic' }],
          needs: needs({ max_life: 5 }),
          summary: '−5 max life, a Mythic relic',
          cost: '5 max life for the run',
          reward: 'a Mythic relic',
        },
        {
          label: 'Burn a card',
          outcomes: ['LoseCard', { RelicOf: 'Rare' }],
          needs: needs(),
          summary: 'give up a card, a Rare relic',
          cost: 'a card of your deck or Satchel',
          reward: 'a Rare relic',
        },
        {
          label: 'Trade a relic',
          outcomes: ['LoseRelic', { Gold: 80 }],
          needs: needs({ relic: true }),
          summary: 'give up a relic, +80 gold',
          cost: 'a relic',
          reward: '80 gold',
        },
        {
          label: 'Weaken your champion',
          outcomes: [{ Bane: { Shrink: 1 } }, { RelicOf: 'Common' }],
          needs: needs({ commander: true, creature_commander: true }),
          summary: 'your commander gets −1/−1, a Common relic',
          cost: 'your commander gets −1/−1',
          reward: 'a Common relic',
        },
        {
          label: 'Tax your champion',
          outcomes: [{ Bane: { Tax: 1 } }, { Gold: 30 }],
          needs: needs({ commander: true }),
          summary: 'your commander costs {1} more, +30 gold',
          cost: 'your commander costs {1} more to cast',
          reward: '30 gold',
        },
      ],
    },
  ],
}

/** What an option asks of a hero: nothing but what `over` names. */
export function needs(over: Partial<Needs> = {}): Needs {
  return {
    gold: 0,
    life: 0,
    max_life: 0,
    relic: false,
    commander: false,
    creature_commander: false,
    ...over,
  }
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
    satchel: [],
    upgrades: [],
    offers: [],
    life_cap: 0,
    lost: [],
    ...over,
  }
}

export function run(over: Partial<Run> = {}): Run {
  return {
    heat: 0,
    content: 1,
    timers: 'relaxed',
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
    fights: [],
    results: [],
    ...over,
  }
}

export function delveView(over: Partial<DelveView> = {}, runOver: Partial<Run> = {}): DelveView {
  return {
    id: 'd1',
    code: 'CODE1',
    seed: 'SEED',
    host: true,
    me: 0,
    run: run(runOver),
    boss_lives: [
      ['skrix_nest', 18],
      ['grimgut', 33],
    ],
    deck_problems: [],
    ...over,
  }
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
