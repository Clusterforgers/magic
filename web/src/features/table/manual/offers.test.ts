// Goal: the menu of a card lists exactly the manual actions the protocol allows for that card, zone and seat at any time, blocks them for a seat out of the game, says why the others cannot be used, and every filled form builds the exact ManualAction payload.
import { describe, expect, it } from 'vitest'
import type { CounterKind, ManualAction, ObjectView } from '@/protocol/generated'
import { card, permanent } from '@/test-utils/cards'
import {
  attachTargets,
  buildAction,
  counterKind,
  EMPTY_DRAFT,
  isAttachment,
  manualMenu,
  damageTargets,
  GAME_OVER,
  OUT_OF_GAME,
  startDraft,
  validCounterName,
  zoneKind,
} from './offers'
import type { Ask, BuildInput, Draft, GroupId, MenuInput } from './offers'

const SEAT = 0

function input(object: ObjectView, over: Partial<MenuInput> = {}): MenuInput {
  return {
    seat: SEAT,
    object,
    players: [0, 1],
    hasCommandZone: true,
    ownerName: 'Bo',
    ...over,
  }
}

const keys = (menu: ReturnType<typeof manualMenu>, group: GroupId): string[] =>
  menu.groups.find((entry) => entry.id === group)?.items.map((item) => item.key) ?? []
const groupIds = (menu: ReturnType<typeof manualMenu>): GroupId[] => menu.groups.map((entry) => entry.id)
const item = (menu: ReturnType<typeof manualMenu>, group: GroupId, key: string) =>
  menu.groups.find((entry) => entry.id === group)?.items.find((entry) => entry.key === key)

const PANEL = ['in-panel']
const MOVES = ['Hand', 'Graveyard', 'Exile', 'LibraryTop', 'LibraryBottom', 'Command'].map(
  (to) => `move-${to}`,
)

describe('which groups and entries a card offers', () => {
  const equipment = permanent({}, { name: 'Sword', types: ['Artifact'], subtypes: ['Equipment'] })
  const cases: [string, ObjectView, Partial<MenuInput>, Partial<Record<GroupId, string[]>>][] = [
    [
      'an untapped creature of the seat',
      card(),
      {},
      {
        move: MOVES,
        counters: ['add-+1/+1', 'add-counters', 'remove-counters'],
        tap: ['tap'],
        effect: ['damage', 'pump'],
        panel: PANEL,
      },
    ],
    [
      'a creature of an opponent',
      card({ owner: 1, controller: 1 }),
      {},
      { tap: ['tap'], effect: ['pump'], panel: PANEL },
    ],
    [
      'a creature with two kinds of counter',
      card({
        counters: [
          ['+1/+1', 2],
          [{ Other: 'oil' }, 1],
        ],
      }),
      {},
      {
        counters: ['add-+1/+1', 'remove-+1/+1', 'add-oil', 'remove-oil', 'add-counters', 'remove-counters'],
      },
    ],
    [
      'a planeswalker without counters',
      card({ types: ['Planeswalker'], subtypes: ['Jace'] }),
      {},
      { counters: ['add-loyalty', 'add-counters', 'remove-counters'] },
    ],
    [
      'a land of the seat',
      card({ types: ['Land'], subtypes: [] }),
      {},
      { counters: ['add-counters', 'remove-counters'], tap: ['tap'], effect: ['damage'] },
    ],
    [
      'a card in the seat hand',
      card({ zone: { Hand: SEAT }, permanent: null }),
      {},
      {
        move: [
          'discard',
          'move-Battlefield',
          'move-Graveyard',
          'move-Exile',
          'move-LibraryTop',
          'move-LibraryBottom',
          'move-Command',
        ],
        panel: PANEL,
      },
    ],
    [
      'a card in the seat graveyard',
      card({ zone: { Graveyard: SEAT }, permanent: null }),
      {},
      {
        move: [
          'move-Battlefield',
          'move-Hand',
          'move-Exile',
          'move-LibraryTop',
          'move-LibraryBottom',
          'move-Command',
        ],
      },
    ],
    [
      'a card in exile',
      card({ zone: 'Exile', permanent: null }),
      {},
      {
        move: [
          'move-Battlefield',
          'move-Hand',
          'move-Graveyard',
          'move-LibraryTop',
          'move-LibraryBottom',
          'move-Command',
        ],
      },
    ],
    [
      'a commander in the command zone',
      card({ zone: { Command: SEAT }, permanent: null }),
      {},
      {
        move: [
          'move-Battlefield',
          'move-Hand',
          'move-Graveyard',
          'move-Exile',
          'move-LibraryTop',
          'move-LibraryBottom',
        ],
      },
    ],
    [
      'a card in a game without a command zone',
      card(),
      { hasCommandZone: false },
      { move: MOVES.filter((key) => key !== 'move-Command') },
    ],
    ['a token on the battlefield', card({ kind: 'Token' }), {}, { move: ['remove-token'], tap: ['tap'] }],
    ['an Equipment of the seat', equipment, {}, { attach: ['attach'], tap: ['tap'] }],
    [
      'an attached Equipment of the seat',
      { ...equipment, attached_to: 7 },
      {},
      { attach: ['attach', 'unattach'] },
    ],
    ['a creature, which attaches to nothing', card(), {}, { attach: [] }],
  ]
  it.each(cases)('%s', (_name, object, over, expected) => {
    const menu = manualMenu(input(object, over))
    for (const [group, wanted] of Object.entries(expected) as [GroupId, string[]][]) {
      expect(keys(menu, group)).toEqual(wanted)
    }
    if (expected.attach?.length === 0) expect(groupIds(menu)).not.toContain('attach')
  })

  it('lists the groups in the order move, counters, tap, attach, effects, show, panel', () => {
    const menu = manualMenu(input({ ...card(), subtypes: ['Aura'] }))
    expect(groupIds(menu)).toEqual(['move', 'counters', 'tap', 'attach', 'effect', 'show', 'panel'])
  })

  it('has no entry for the seat’s life, counters, library or mana, nor for a token: the panel has them', () => {
    const menu = manualMenu(input(card()))
    const labels = menu.groups.flatMap((group) => group.items.map((entry) => entry.label.toLowerCase()))
    expect(labels.filter((label) => /life|draw|mill|token|mana|library top/.test(label))).toEqual([])
    expect(menu.groups.flatMap((group) => group.items.map((entry) => entry.ask))).not.toContain('token')
  })

  it('leaves out counters, tapping and attaching for a card that is not a permanent', () => {
    const menu = manualMenu(input(card({ zone: { Hand: SEAT }, permanent: null, subtypes: ['Aura'] })))
    expect(groupIds(menu)).toEqual(['move', 'show', 'panel'])
  })
})

describe('what stops an entry', () => {
  it('an instant or sorcery cannot enter the battlefield', () => {
    const spell = card({ zone: { Hand: SEAT }, types: ['Instant'], permanent: null })
    const menu = manualMenu(input(spell))
    expect(item(menu, 'move', 'move-Battlefield')?.unavailable).toMatch(/instant or sorcery/)
    expect(item(menu, 'move', 'move-Graveyard')?.unavailable).toBeUndefined()
  })

  it('names the owner of a card the seat only controls', () => {
    const stolen = card({ owner: 1, controller: SEAT })
    const labels = manualMenu(input(stolen))
      .groups.find((entry) => entry.id === 'move')
      ?.items.map((entry) => entry.label)
    expect(labels).toContain("Bo's hand")
    expect(labels).toContain('Exile')
    expect(labels).not.toContain('Battlefield')
    expect(labels).not.toContain('Graveyard')
  })

  it.each([
    ['a card of an opponent', card({ owner: 1, controller: 1 }), 'move', /Only Bo/],
    ['a spell on the stack', card({ zone: 'Stack', permanent: null }), 'move', /stack/],
    [
      'an Equipment of an opponent',
      card({ owner: 1, controller: 1, subtypes: ['Equipment'] }),
      'attach',
      /Only Bo/,
    ],
  ] as const)('%s has a note and no entry', (_name, object, group, note) => {
    const found = manualMenu(input(object)).groups.find((entry) => entry.id === group)
    expect(found?.items).toEqual([])
    expect(found?.note).toMatch(note)
  })

  it('an opponent card still offers counters, tapping and the entries about the seat itself', () => {
    const menu = manualMenu(input(card({ owner: 1, controller: 1 })))
    expect(keys(menu, 'tap')).toEqual(['tap'])
    expect(keys(menu, 'counters')).toContain('add-counters')
    expect(keys(menu, 'panel')).toEqual(PANEL)
  })

  it.each([OUT_OF_GAME, GAME_OVER])('once %s every entry is listed and none can be used', (reason) => {
    const menu = manualMenu(input(card({ counters: [['+1/+1', 1]] }), { off: reason }))
    expect(menu.notice).toBe(reason)
    const all = menu.groups.flatMap((entry) => entry.items)
    expect(all.length).toBeGreaterThan(10)
    expect(all.every((entry) => entry.unavailable === reason)).toBe(true)
  })

  it('for a seat in the game nothing is blocked on an ordinary permanent but removing counters it does not have', () => {
    const menu = manualMenu(input(card()))
    expect(menu.notice).toBeUndefined()
    const blocked = menu.groups.flatMap((entry) => entry.items).filter((entry) => entry.unavailable)
    expect(blocked.map((entry) => entry.key).sort()).toEqual(['remove-counters'])
  })
})

describe('the actions an entry sends as it is', () => {
  const sends: [string, ObjectView, GroupId, string, ManualAction][] = [
    ['a move', card({ id: 3 }), 'move', 'move-Exile', { Move: { object: 3, to: 'Exile' } }],
    [
      'a move to the top of the library',
      card({ id: 3 }),
      'move',
      'move-LibraryTop',
      { Move: { object: 3, to: 'LibraryTop' } },
    ],
    [
      'a discard',
      card({ id: 3, zone: { Hand: SEAT }, permanent: null }),
      'move',
      'discard',
      { Discard: { object: 3 } },
    ],
    ['a tap', card({ id: 3 }), 'tap', 'tap', { Tap: { object: 3 } }],
    ['an untap', permanent({ tapped: true }, { id: 3 }), 'tap', 'tap', { Untap: { object: 3 } }],
    [
      'one more counter',
      card({ id: 3, counters: [['+1/+1', 2]] }),
      'counters',
      'add-+1/+1',
      { Counters: { object: 3, counter: '+1/+1', delta: 1 } },
    ],
    [
      'one counter fewer',
      card({ id: 3, counters: [[{ Other: 'oil' }, 2]] }),
      'counters',
      'remove-oil',
      { Counters: { object: 3, counter: { Other: 'oil' }, delta: -1 } },
    ],
    [
      'the removal of a token',
      card({ id: 3, kind: 'Token' }),
      'move',
      'remove-token',
      { Move: { object: 3, to: 'Graveyard' } },
    ],
    [
      'an unattach',
      card({ id: 3, subtypes: ['Equipment'], attached_to: 9 }),
      'attach',
      'unattach',
      { Unattach: { object: 3 } },
    ],
  ]
  it.each(sends)('%s', (_name, object, group, key, action) => {
    expect(item(manualMenu(input(object)), group, key)?.action).toEqual(action)
  })
})

describe('the forms', () => {
  const object = card({
    id: 3,
    counters: [
      ['+1/+1', 3],
      [{ Other: 'oil' }, 2],
    ],
  })
  const base = (
    over: Partial<Draft>,
    extra: Partial<MenuInput> = {},
  ): Omit<BuildInput, 'draft'> & { draft: Draft } => ({
    ...input(object, extra),
    draft: { ...EMPTY_DRAFT, ...over },
  })
  const built = (ask: Ask, over: Partial<Draft>, extra: Partial<MenuInput> = {}) =>
    buildAction(ask, base(over, extra))

  const ok: [string, Ask, Partial<Draft>, Partial<MenuInput>, ManualAction][] = [
    [
      'counters added',
      'addCounters',
      { count: 3 },
      {},
      { Counters: { object: 3, counter: '+1/+1', delta: 3 } },
    ],
    [
      'counters of a new kind',
      'addCounters',
      { kind: 'other', other: ' time ', count: 2 },
      {},
      { Counters: { object: 3, counter: { Other: 'time' }, delta: 2 } },
    ],
    [
      'a known kind typed as other',
      'addCounters',
      { kind: 'other', other: 'poison', count: 1 },
      {},
      { Counters: { object: 3, counter: 'poison', delta: 1 } },
    ],
    [
      'counters removed',
      'removeCounters',
      { kind: 'oil', count: 2 },
      {},
      { Counters: { object: 3, counter: { Other: 'oil' }, delta: -2 } },
    ],
    ['an attachment', 'attach', { target: 12 }, {}, { Attach: { object: 3, to: 12 } }],
    [
      'damage to a player',
      'damage',
      { hit: { Player: 1 }, count: 3 },
      {},
      { Damage: { source: 3, target: { Player: 1 }, amount: 3 } },
    ],
    [
      'damage to a creature',
      'damage',
      { hit: { Object: 12 }, count: 99 },
      {},
      { Damage: { source: 3, target: { Object: 12 }, amount: 99 } },
    ],
    ['a pump', 'pump', { power: 3, toughness: 0 }, {}, { Pump: { object: 3, power: 3, toughness: 0 } }],
    [
      'a shrink',
      'pump',
      { power: -99, toughness: -2 },
      {},
      { Pump: { object: 3, power: -99, toughness: -2 } },
    ],
  ]
  it.each(ok)('builds %s', (_name, ask, draft, extra, action) => {
    expect(built(ask, draft, extra)).toEqual({ action })
  })

  const bad: [string, Ask, Partial<Draft>, Partial<MenuInput>][] = [
    ['zero counters', 'addCounters', { count: 0 }, {}],
    ['100 counters', 'addCounters', { count: 100 }, {}],
    ['a fraction', 'addCounters', { count: 1.5 }, {}],
    ['no number', 'addCounters', { count: Number.NaN }, {}],
    ['a counter name with punctuation', 'addCounters', { kind: 'other', other: 'a!b' }, {}],
    ['an empty counter name', 'addCounters', { kind: 'other', other: '  ' }, {}],
    ['a counter name of 25 characters', 'addCounters', { kind: 'other', other: 'x'.repeat(25) }, {}],
    ['more counters than there are', 'removeCounters', { kind: 'oil', count: 3 }, {}],
    ['a kind that is not there', 'removeCounters', { kind: 'charge' }, {}],
    ['no target', 'attach', {}, {}],
    ['damage to nothing', 'damage', { count: 2 }, {}],
    ['zero damage', 'damage', { hit: { Player: 1 }, count: 0 }, {}],
    ['100 damage', 'damage', { hit: { Player: 1 }, count: 100 }, {}],
    ['a pump of nothing', 'pump', { power: 0, toughness: 0 }, {}],
    ['a pump of 100', 'pump', { power: 100, toughness: 1 }, {}],
    ['a shrink of 100', 'pump', { power: 1, toughness: -100 }, {}],
    ['a fractional pump', 'pump', { power: 0.5, toughness: 1 }, {}],
  ]
  it.each(bad)('refuses %s with a reason', (_name, ask, draft, extra) => {
    const result = built(ask, draft, extra)
    expect(result).toHaveProperty('problem')
  })

  it('starts removal on a kind that is held', () => {
    const start = (ask: Ask, over: Partial<ObjectView> = {}) =>
      startDraft(ask, input({ ...object, ...over })).kind
    expect(start('removeCounters')).toBe('+1/+1')
    expect(start('removeCounters', { counters: [[{ Other: 'energy' }, 1]] })).toBe('energy')
    expect(start('addCounters', { counters: [[{ Other: 'energy' }, 1]] })).toBe('+1/+1')
  })
})

describe('what damage can be dealt to', () => {
  it('lists the players in the game, the other seats first, then what takes damage on the battlefield by name', () => {
    const objects = [
      card({ id: 1, name: 'Zombie', types: ['Creature'] }),
      card({ id: 2, name: 'Jace', types: ['Planeswalker'] }),
      card({ id: 3, name: 'Invasion', types: ['Battle'] }),
      card({ id: 4, name: 'Forest', types: ['Land'] }),
      card({ id: 5, name: 'Bear', types: ['Creature'], zone: { Graveyard: 0 }, permanent: null }),
    ]
    expect(damageTargets(objects, [0, 2, 3], 2)).toEqual([
      { Player: 0 },
      { Player: 3 },
      { Player: 2 },
      { Object: 3 },
      { Object: 2 },
      { Object: 1 },
    ])
  })
})

describe('small rules', () => {
  it('names the zone kind of every zone', () => {
    const zones: [Parameters<typeof zoneKind>[0], ReturnType<typeof zoneKind>][] = [
      ['Battlefield', 'battlefield'],
      ['Exile', 'exile'],
      ['Stack', 'stack'],
      ['PhasedOut', 'phased'],
      [{ Hand: 1 }, 'hand'],
      [{ Graveyard: 1 }, 'graveyard'],
      [{ Library: 1 }, 'library'],
      [{ Command: 1 }, 'command'],
    ]
    for (const [zone, kind] of zones) expect(zoneKind(zone)).toBe(kind)
  })

  it('takes the counter names the protocol takes', () => {
    expect(['+1/+1', 'oil', 'time 2', 'a-b', 'x'.repeat(24)].every(validCounterName)).toBe(true)
    expect(['', 'a!', 'x'.repeat(25), 'a_b'].some(validCounterName)).toBe(false)
    expect(counterKind('loyalty')).toBe('loyalty')
    expect(counterKind('oil')).toEqual({ Other: 'oil' } satisfies CounterKind)
  })

  it('knows an Aura, an Equipment and a Fortification', () => {
    expect(
      ['Aura', 'Equipment', 'Fortification'].every((kind) => isAttachment(card({ subtypes: [kind] }))),
    ).toBe(true)
    expect(isAttachment(card())).toBe(false)
  })

  it('lists attach targets: not itself, not its host, the seat first, then by name', () => {
    const sword = card({ id: 1, name: 'Sword', subtypes: ['Equipment'], attached_to: 4 })
    const objects = [
      sword,
      card({ id: 2, name: 'Zed', controller: 1 }),
      card({ id: 3, name: 'Bear' }),
      card({ id: 4, name: 'Host' }),
      card({ id: 5, name: 'Aaa', controller: 1 }),
      card({ id: 6, name: 'Hand card', zone: { Hand: 0 }, permanent: null }),
      card({ id: 7, name: 'Trigger', kind: 'Ability' }),
    ]
    expect(attachTargets(objects, sword, SEAT).map((entry) => entry.name)).toEqual(['Bear', 'Aaa', 'Zed'])
  })
})
