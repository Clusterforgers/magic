// Goal: groupPermanents merges permanents with the same face (name, owner, controller, tapped state, summoning sickness, damage, power and toughness, types, colors, abilities, counters and attachments) into one stack with a count and keeps every other pair apart, always merges lands and tokens but other cards only with groupOthers, orders stacks by their first member, keeps a stack's key while any member stays and every other stack's key, count and place when one member joins or leaves, and expandStack gives the members back so that regrouping them gives the same stacks.
import { describe, expect, it } from 'vitest'
import type { ObjectView, PermanentView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { card, forest } from '@/test-utils/cards'
import { FIT_LIMITS, LAND_SCALE, fitRows } from './fit'
import { GROUP_OTHERS_BELOW, expandStack, groupPermanents } from './group'
import type { Groupable, GroupOptions, Stack } from './group'

/** A small seeded generator, so a failing case can be replayed. */
function mulberry32(seed: number): () => number {
  let state = seed
  return () => {
    state = (state + 0x6d2b79f5) | 0
    let t = Math.imul(state ^ (state >>> 15), 1 | state)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/** A permanent as the wire sends it, plus the counters and attachments the wire will send. */
type Perm = ObjectView & Pick<Groupable, 'counters' | 'attachments' | 'role'>

const land = (id: number, over: Partial<Perm> = {}): Perm => ({ ...forest({ id }), ...over })
const bear = (id: number, over: Partial<Perm> = {}): Perm => ({ ...card({ id }), ...over })
const goblin = (id: number, over: Partial<Perm> = {}): Perm => ({
  ...card({
    id,
    name: 'Goblin',
    kind: 'Token',
    mana_cost: null,
    colors: ['Red'],
    subtypes: ['Goblin'],
    power: 1,
    toughness: 1,
  }),
  ...over,
})
const state = (over: Partial<PermanentView>): PermanentView => ({
  tapped: false,
  damage: 0,
  summoning_sick: false,
  ...over,
})
const tapped = <T extends Perm>(object: T): T => ({ ...object, permanent: state({ tapped: true }) })

const ids = (stacks: readonly Stack[]): number[][] => stacks.map((stack) => stack.members.map((m) => m.id))
const keys = (stacks: readonly Stack[]): string[] => stacks.map((stack) => stack.key)

describe('what merges and what stays apart', () => {
  const BASES: [string, (id: number, over?: Partial<Perm>) => Perm, GroupOptions][] = [
    ['lands', land, {}],
    ['tokens', goblin, {}],
    ['cards with groupOthers', bear, { groupOthers: true }],
  ]
  // [difference, how the first and the second permanent differ from the base, whether they merge]
  type Make = (base: Perm) => Partial<Perm>
  const none: Make = () => ({})
  const PAIRS: [string, Make, Make, boolean][] = [
    ['nothing but the id', none, none, true],
    ['the name', none, () => ({ name: 'Island' }), false],
    ['the owner', none, () => ({ owner: 1 }), false],
    ['the controller', none, () => ({ controller: 1 }), false],
    ['the tapped state', none, () => ({ permanent: state({ tapped: true }) }), false],
    ['summoning sickness', none, () => ({ permanent: state({ summoning_sick: true }) }), false],
    ['damage', none, () => ({ permanent: state({ damage: 1 }) }), false],
    [
      'both tapped and both sick',
      () => ({ permanent: state({ tapped: true, summoning_sick: true }) }),
      () => ({ permanent: state({ tapped: true, summoning_sick: true }) }),
      true,
    ],
    ['power', none, () => ({ power: 9 }), false],
    ['toughness', none, () => ({ toughness: 9 }), false],
    ['a card type', none, (base) => ({ types: [...base.types, 'Artifact'] }), false],
    ['a color', none, (base) => ({ colors: [...base.colors, 'Black'] }), false],
    [
      'an ability',
      none,
      (base) => ({ abilities: [...base.abilities, { Mana: { produces: 'Black' } }] }),
      false,
    ],
    ['a part in combat', none, () => ({ role: 'attacking' }), false],
    ['the same part in combat', () => ({ role: 'blocking' }), () => ({ role: 'blocking' }), false],
    ['no part in combat on either', () => ({ role: null }), () => ({ role: null }), true],
    ['a keyword', none, () => ({ keywords: ['Flying'] }), false],
    ['other keywords', () => ({ keywords: ['Flying'] }), () => ({ keywords: ['Reach'] }), false],
    ['the same keywords', () => ({ keywords: ['Flying'] }), () => ({ keywords: ['Flying'] }), true],
    ['being a token or a card', none, (base) => ({ kind: base.kind === 'Token' ? 'Card' : 'Token' }), false],
    ['having a counter or none', none, () => ({ counters: [['+1/+1', 1]] }), false],
    ['how many counters', () => ({ counters: [['+1/+1', 1]] }), () => ({ counters: [['+1/+1', 2]] }), false],
    [
      'the kind of counter',
      () => ({ counters: [['+1/+1', 1]] }),
      () => ({ counters: [['-1/-1', 1]] }),
      false,
    ],
    [
      'a named counter',
      () => ({ counters: [[{ Other: 'oil' }, 1]] }),
      () => ({ counters: [[{ Other: 'time' }, 1]] }),
      false,
    ],
    [
      'the same named counter',
      () => ({ counters: [[{ Other: 'oil' }, 1]] }),
      () => ({ counters: [[{ Other: 'oil' }, 1]] }),
      true,
    ],
    [
      'equal counters listed in another order',
      () => ({
        counters: [
          ['+1/+1', 1],
          ['charge', 2],
        ],
      }),
      () => ({
        counters: [
          ['charge', 2],
          ['+1/+1', 1],
        ],
      }),
      true,
    ],
    [
      'equal counters listed in pieces',
      () => ({ counters: [['+1/+1', 2]] }),
      () => ({
        counters: [
          ['+1/+1', 1],
          ['+1/+1', 1],
        ],
      }),
      true,
    ],
    ['a counter of zero and no counters', none, () => ({ counters: [['charge', 0]] }), true],
    ['an attachment or none', none, () => ({ attachments: [90] }), false],
    ['different attachments', () => ({ attachments: [90] }), () => ({ attachments: [91] }), false],
    [
      'equal attachments listed in another order',
      () => ({ attachments: [90, 91] }),
      () => ({ attachments: [91, 90] }),
      true,
    ],
  ]

  describe.each(BASES)('%s', (_, make, options) => {
    it.each(PAIRS)('%s', (_difference, first, second, merges) => {
      const base = make(0)
      const a = make(1, first(base))
      const b = make(2, second(base))
      for (const order of [
        [a, b],
        [b, a],
      ]) {
        const stacks = groupPermanents(order, options)
        if (merges) {
          expect(stacks).toHaveLength(1)
          expect(stacks[0]).toMatchObject({ count: 2, members: order })
        } else {
          expect(stacks.map((stack) => stack.count)).toEqual([1, 1])
          expect(new Set(keys(stacks)).size).toBe(2)
        }
      }
    })
  })
})

describe('which cards merge without being asked', () => {
  const KINDS: [string, (id: number) => Perm, boolean][] = [
    ['a basic land', (id) => land(id), true],
    [
      'a land that is also a creature',
      (id) => bear(id, { name: 'Dryad Arbor', types: ['Land', 'Creature'] }),
      true,
    ],
    ['a token creature', (id) => goblin(id), true],
    [
      'a token that is an artifact',
      (id) => goblin(id, { name: 'Treasure', types: ['Artifact'], power: null, toughness: null }),
      true,
    ],
    ['a creature card', (id) => bear(id), false],
    [
      'an artifact card',
      (id) => bear(id, { name: 'Sol Ring', types: ['Artifact'], power: null, toughness: null }),
      false,
    ],
    [
      'an enchantment card',
      (id) => bear(id, { name: 'Rancor', types: ['Enchantment'], power: null, toughness: null }),
      false,
    ],
    [
      'a planeswalker card',
      (id) => bear(id, { name: 'Jace', types: ['Planeswalker'], power: null, toughness: null }),
      false,
    ],
  ]

  it.each(KINDS)('%s', (_, make, always) => {
    const pair = [make(1), make(2)]
    const plain = groupPermanents(pair)
    const asked = groupPermanents(pair, { groupOthers: true })
    expect(ids(plain)).toEqual(always ? [[1, 2]] : [[1], [2]])
    expect(ids(asked)).toEqual([[1, 2]])
    expect(groupPermanents(pair, { groupOthers: false })).toEqual(plain)
  })

  it('names a card that stays alone by its id, so its key does not change when it is moved', () => {
    expect(keys(groupPermanents([bear(5), bear(9)]))).toEqual(['#5', '#9'])
    expect(keys(groupPermanents([bear(9), bear(5)]))).toEqual(['#9', '#5'])
  })

  it(`sets the threshold at which the table asks for it: ${GROUP_OTHERS_BELOW} px`, () => {
    expect(GROUP_OTHERS_BELOW).toBe(64)
  })
})

describe('order and state', () => {
  const [f, i, m] = [land(0), land(0, { name: 'Island' }), land(0, { name: 'Mountain' })]
  const at = (object: Perm, id: number, over: Partial<Perm> = {}): Perm => ({ ...object, id, ...over })

  it.each([
    ['empty', [], []],
    ['one', [at(f, 1)], [[1]]],
    [
      'stacks go by their first member',
      [at(f, 1), at(i, 2), at(f, 3), at(m, 4), at(i, 5)],
      [[1, 3], [2, 5], [4]],
    ],
    [
      'tapped lands stay apart from untapped ones',
      [at(f, 1), tapped(at(f, 2)), at(f, 3), tapped(at(f, 4))],
      [
        [1, 3],
        [2, 4],
      ],
    ],
    [
      'a tapped first member leads its own stack',
      [tapped(at(f, 1)), at(f, 2), tapped(at(f, 3))],
      [[1, 3], [2]],
    ],
    ['controllers stay apart', [at(f, 1), at(f, 2, { controller: 1 }), at(f, 3)], [[1, 3], [2]]],
    [
      'summoning sickness stays apart',
      [at(f, 1, { permanent: state({ summoning_sick: true }) }), at(f, 2), at(f, 3)],
      [[1], [2, 3]],
    ],
  ] as [string, Perm[], number[][]][])('%s', (_, list, expected) => {
    expect(ids(groupPermanents(list))).toEqual(expected)
  })

  it('counts every member of a stack and none twice', () => {
    const stacks = groupPermanents([at(f, 1), at(f, 2), tapped(at(f, 3)), at(i, 4), at(f, 5)])
    expect(stacks.map((stack) => [stack.count, stack.members.length])).toEqual([
      [3, 3],
      [1, 1],
      [1, 1],
    ])
  })

  it('does not change what it is given', () => {
    const list = Object.freeze([at(f, 1), tapped(at(f, 2)), at(i, 3, { counters: [['+1/+1', 1]] })])
    const before = structuredClone(list)
    groupPermanents(list, { groupOthers: true })
    expect(list).toEqual(before)
  })

  it('names a merged stack by its face alone: the same for other ids and places, different for another face', () => {
    const key = (...list: Perm[]): string => must(groupPermanents(list)[0]).key
    expect(key(at(f, 1))).toBe(key(at(f, 40), at(f, 41)))
    expect(key(at(f, 1))).not.toBe(key(at(i, 1)))
    expect(key(at(f, 1))).not.toBe(key(tapped(at(f, 1))))
    expect(key(at(f, 1))).not.toBe(key(at(f, 1, { controller: 1 })))
    expect(key(at(f, 1))).not.toMatch(/^#/)
  })
})

/** The keys and members of the stacks of one list set against another: what was added, removed or changed. */
function diffStacks(before: readonly Stack[], after: readonly Stack[]) {
  const was = new Map(before.map((stack) => [stack.key, stack]))
  const is = new Map(after.map((stack) => [stack.key, stack]))
  return {
    added: after.filter((stack) => !was.has(stack.key)).map((stack) => stack.key),
    removed: before.filter((stack) => !is.has(stack.key)).map((stack) => stack.key),
    changed: after
      .filter((stack) => {
        const old = was.get(stack.key)
        return (
          old !== undefined &&
          JSON.stringify(old.members.map((m) => m.id)) !== JSON.stringify(stack.members.map((m) => m.id))
        )
      })
      .map((stack) => stack.key),
  }
}

/** The stacks that are in both lists keep the order they had. */
function sameRelativeOrder(
  before: readonly Stack[],
  after: readonly Stack[],
  except: readonly string[],
): boolean {
  const left = keys(before).filter((key) => !except.includes(key) && keys(after).includes(key))
  const right = keys(after).filter((key) => !except.includes(key) && keys(before).includes(key))
  return JSON.stringify(left) === JSON.stringify(right)
}

describe('keys and counts when one member joins or leaves', () => {
  const OPTIONS = { groupOthers: true }
  const swamp = land(9, { name: 'Swamp' })
  // Forests 1 and 3, an Island, a tapped Forest, two Bears and two Goblin tokens.
  const BASE: Perm[] = [
    land(1),
    land(2, { name: 'Island' }),
    land(3),
    tapped(land(4)),
    bear(5),
    goblin(6),
    bear(7),
    goblin(8),
  ]
  const before = groupPermanents(BASE, OPTIONS)
  const keyOf = (object: Perm): string => must(groupPermanents([object], OPTIONS)[0]).key
  const [forests, island, tappedForest, bears, goblins] = [
    keyOf(land(0)),
    keyOf(land(0, { name: 'Island' })),
    keyOf(tapped(land(0))),
    keyOf(bear(0)),
    keyOf(goblin(0)),
  ]
  const without = (id: number): Perm[] => BASE.filter((object) => object.id !== id)
  const withAt = (index: number, object: Perm): Perm[] => [
    ...BASE.slice(0, index),
    object,
    ...BASE.slice(index),
  ]

  it('starts from five stacks in the order of their first members', () => {
    expect(keys(before)).toEqual([forests, island, tappedForest, bears, goblins])
    expect(before.map((stack) => stack.count)).toEqual([2, 1, 1, 2, 2])
  })

  it.each([
    ['a forest joins at the end', [...BASE, land(10)], [forests], 3],
    ['a forest joins in the middle', withAt(3, land(10)), [forests], 3],
    ['a forest joins at the front', withAt(0, land(10)), [forests], 3],
    ['a forest that is not first leaves', without(3), [forests], 1],
    ['the first forest leaves', without(1), [forests], 1],
    ['a bear leaves', without(7), [bears], 1],
    ['a goblin token joins', [...BASE, goblin(10)], [goblins], 3],
  ] as [string, Perm[], string[], number][])(
    '%s: only that stack changes, its key stays',
    (_, list, changed, count) => {
      const after = groupPermanents(list, OPTIONS)
      expect(diffStacks(before, after)).toEqual({ added: [], removed: [], changed })
      expect(after.find((stack) => stack.key === changed[0])?.count).toBe(count)
      expect(sameRelativeOrder(before, after, changed)).toBe(true)
    },
  )

  it('removes a key only with the last member of its stack', () => {
    expect(diffStacks(before, groupPermanents(without(2), OPTIONS))).toEqual({
      added: [],
      removed: [island],
      changed: [],
    })
    expect(diffStacks(before, groupPermanents(without(4), OPTIONS))).toEqual({
      added: [],
      removed: [tappedForest],
      changed: [],
    })
    expect(sameRelativeOrder(before, groupPermanents(without(2), OPTIONS), [island])).toBe(true)
  })

  it('adds a key only for a face no stack has, at the place of its first member', () => {
    const after = groupPermanents([...BASE, swamp], OPTIONS)
    expect(diffStacks(before, after)).toEqual({ added: [keyOf(swamp)], removed: [], changed: [] })
    expect(keys(after).at(-1)).toBe(keyOf(swamp))
    const first = groupPermanents(withAt(0, swamp), OPTIONS)
    expect(keys(first)[0]).toBe(keyOf(swamp))
    expect(diffStacks(before, first).added).toEqual([keyOf(swamp)])
  })

  it('moves one permanent from the stack it left to the one it joined when it taps', () => {
    const taps = BASE.map((object) => (object.id === 3 ? tapped(object) : object))
    const after = groupPermanents(taps, OPTIONS)
    expect(diffStacks(before, after)).toEqual({ added: [], removed: [], changed: [forests, tappedForest] })
    expect(ids(after)).toEqual([[1], [2], [3, 4], [5, 7], [6, 8]])
  })

  it('keeps a card that stays alone under its id while others come and go', () => {
    const solo = groupPermanents(BASE)
    const soloBears = solo.filter((stack) => stack.key.startsWith('#'))
    expect(soloBears.map((stack) => stack.key)).toEqual(['#5', '#7'])
    const after = groupPermanents(without(5))
    expect(after.filter((stack) => stack.key.startsWith('#')).map((stack) => stack.key)).toEqual(['#7'])
  })
})

describe('expandStack', () => {
  const list: Perm[] = [land(1), bear(2), land(3), tapped(land(4)), goblin(5), land(6), goblin(7), bear(8)]

  it('gives the members of a stack in the order they were given', () => {
    const [forests, bearOne, tappedForest, goblins, bearTwo] = groupPermanents(list)
    expect(expandStack(must(forests)).map((m) => m.id)).toEqual([1, 3, 6])
    expect(expandStack(must(bearOne)).map((m) => m.id)).toEqual([2])
    expect(expandStack(must(tappedForest)).map((m) => m.id)).toEqual([4])
    expect(expandStack(must(goblins)).map((m) => m.id)).toEqual([5, 7])
    expect(expandStack(must(bearTwo)).map((m) => m.id)).toEqual([8])
  })

  it('gives every permanent exactly once, grouped by stack, for a prompt that needs each one', () => {
    const stacks = groupPermanents(list, { groupOthers: true })
    const each = stacks.flatMap(expandStack)
    expect(each.map((m) => m.id)).toEqual([1, 3, 6, 2, 8, 4, 5, 7])
    expect(new Set(each.map((m) => m.id))).toEqual(new Set(list.map((m) => m.id)))
  })

  it('opens one stack in place: its members take the stack`s place and the rest stays', () => {
    const stacks = groupPermanents(list, { groupOthers: true })
    const open = must(stacks[0])
    const row = stacks.flatMap((stack) =>
      stack === open ? expandStack(stack).map((member) => [member]) : [stack.members],
    )
    expect(row.map((members) => members.map((m) => m.id))).toEqual([[1], [3], [6], [2, 8], [4], [5, 7]])
  })

  it('regroups to the same stacks, keys and counts', () => {
    for (const options of [{}, { groupOthers: true }]) {
      const stacks = groupPermanents(list, options)
      const again = groupPermanents(stacks.flatMap(expandStack), options)
      expect(again).toEqual(stacks)
    }
  })
})

/** A kind of permanent the random lists draw from, and how it merges. */
interface Face {
  label: string
  make: (id: number) => Perm
  merges: 'always' | 'cards' | 'never'
}

const FACES: Face[] = [
  { label: 'forest', make: (id) => land(id), merges: 'always' },
  { label: 'forest, tapped', make: (id) => tapped(land(id)), merges: 'always' },
  { label: 'island', make: (id) => land(id, { name: 'Island' }), merges: 'always' },
  { label: 'goblin', make: (id) => goblin(id), merges: 'always' },
  { label: 'goblin, tapped', make: (id) => tapped(goblin(id)), merges: 'always' },
  { label: 'bear', make: (id) => bear(id), merges: 'cards' },
  { label: 'bear, tapped', make: (id) => tapped(bear(id)), merges: 'cards' },
  {
    label: 'bear, sick',
    make: (id) => bear(id, { permanent: state({ summoning_sick: true }) }),
    merges: 'cards',
  },
  { label: 'bear, counter', make: (id) => bear(id, { counters: [['+1/+1', 1]] }), merges: 'cards' },
  { label: 'bear, yours', make: (id) => bear(id, { controller: 1 }), merges: 'cards' },
  {
    label: 'angel',
    make: (id) => bear(id, { name: 'Serra Angel', power: 4, toughness: 4 }),
    merges: 'cards',
  },
  { label: 'bear, equipped', make: (id) => bear(id, { attachments: [1000 + id] }), merges: 'never' },
]

/** Which stack a permanent belongs to, by the face it was drawn from; the truth the random tests check against. */
function expectedGroups(list: readonly { face: Face; perm: Perm }[], groupOthers: boolean): number[][] {
  const groups = new Map<string, number[]>()
  for (const { face, perm } of list) {
    const alone = face.merges === 'never' || (face.merges === 'cards' && !groupOthers)
    const label = alone ? `#${perm.id}` : face.label
    groups.set(label, [...(groups.get(label) ?? []), perm.id])
  }
  return [...groups.values()]
}

describe('seeded random lists', () => {
  const random = mulberry32(8675309)
  const int = (low: number, high: number): number => low + Math.floor(random() * (high - low + 1))
  const draw = (id: number): { face: Face; perm: Perm } => {
    const face = must(FACES[int(0, FACES.length - 1)])
    return { face, perm: face.make(id) }
  }
  const LISTS = Array.from({ length: 300 }, () => {
    const length = int(0, 40)
    return Array.from({ length }, (_, index) => draw(index + 1))
  })

  it.each([false, true])('partitions every list the way the faces say (groupOthers %s)', (groupOthers) => {
    for (const [index, list] of LISTS.entries()) {
      const stacks = groupPermanents(
        list.map((entry) => entry.perm),
        { groupOthers },
      )
      expect(ids(stacks), `list ${index}`).toEqual(expectedGroups(list, groupOthers))
      expect(
        stacks.map((stack) => stack.count),
        `list ${index}`,
      ).toEqual(stacks.map((stack) => stack.members.length))
      expect(new Set(keys(stacks)).size, `list ${index}: keys are unique`).toBe(stacks.length)
    }
  })

  it.each([false, true])('keeps keys when the list is shuffled (groupOthers %s)', (groupOthers) => {
    for (const [index, list] of LISTS.entries()) {
      const perms = list.map((entry) => entry.perm)
      const shuffled = [...perms].sort(() => random() - 0.5)
      expect([...keys(groupPermanents(shuffled, { groupOthers }))].sort(), `list ${index}`).toEqual(
        [...keys(groupPermanents(perms, { groupOthers }))].sort(),
      )
    }
  })

  it.each([false, true])(
    'changes one stack at most when one member joins or leaves (groupOthers %s)',
    (groupOthers) => {
      const options = { groupOthers }
      for (const [index, list] of LISTS.entries()) {
        const perms = list.map((entry) => entry.perm)
        const before = groupPermanents(perms, options)
        const label = `list ${index}`

        const joined = [...perms]
        joined.splice(int(0, joined.length), 0, draw(1000 + index).perm)
        const afterJoin = groupPermanents(joined, options)
        const join = diffStacks(before, afterJoin)
        expect(join.removed, `${label}: join removes no key`).toEqual([])
        expect(join.added.length + join.changed.length, `${label}: join touches one stack`).toBe(1)
        expect(
          sameRelativeOrder(before, afterJoin, [...join.added, ...join.changed]),
          `${label}: join order`,
        ).toBe(true)

        if (perms.length === 0) continue
        const left = [...perms]
        left.splice(int(0, left.length - 1), 1)
        const afterLeave = groupPermanents(left, options)
        const leave = diffStacks(before, afterLeave)
        expect(leave.added, `${label}: leave adds no key`).toEqual([])
        expect(leave.removed.length + leave.changed.length, `${label}: leave touches one stack`).toBe(1)
        expect(
          sameRelativeOrder(before, afterLeave, [...leave.removed, ...leave.changed]),
          `${label}: leave order`,
        ).toBe(true)
      }
    },
  )

  it.each([false, true])('regroups what it expanded to the same stacks (groupOthers %s)', (groupOthers) => {
    for (const [index, list] of LISTS.entries()) {
      const stacks = groupPermanents(
        list.map((entry) => entry.perm),
        { groupOthers },
      )
      const flat = stacks.flatMap(expandStack)
      expect(flat, `list ${index}`).toHaveLength(list.length)
      expect(groupPermanents(flat, { groupOthers }), `list ${index}`).toEqual(stacks)
    }
  })
})

describe('with the fit engine', () => {
  // A seat with 20 creatures of four kinds, 6 other permanents of three and 24 lands of two, some tapped.
  const creatures: Perm[] = [
    ...Array.from({ length: 8 }, (_, n) => bear(100 + n, { name: 'Llanowar Elves', power: 1, toughness: 1 })),
    ...Array.from({ length: 6 }, (_, n) => bear(200 + n)),
    ...Array.from({ length: 4 }, (_, n) => bear(300 + n, { name: 'Serra Angel', power: 4, toughness: 4 })),
    ...Array.from({ length: 2 }, (_, n) => bear(400 + n, { name: 'Hill Giant', power: 3, toughness: 3 })),
  ]
  const others: Perm[] = [
    ...Array.from({ length: 3 }, (_, n) =>
      bear(500 + n, { name: 'Mind Stone', types: ['Artifact'], power: null, toughness: null }),
    ),
    ...Array.from({ length: 2 }, (_, n) =>
      bear(600 + n, { name: 'Sol Ring', types: ['Artifact'], power: null, toughness: null }),
    ),
    bear(700, { name: 'Rancor', types: ['Enchantment'], power: null, toughness: null }),
  ]
  const lands: Perm[] = Array.from({ length: 24 }, (_, n) => {
    const object = land(800 + n, n % 2 === 0 ? {} : { name: 'Island' })
    return n % 3 === 0 ? tapped(object) : object
  })

  function fitSeat(groupOthers: boolean, box: { width: number; height: number }) {
    const rows = [
      groupPermanents(creatures, { groupOthers }),
      groupPermanents(others, { groupOthers }),
      groupPermanents(lands, { groupOthers }),
    ]
    return fitRows(box, [
      { stacks: rows[0]?.length ?? 0 },
      { stacks: rows[1]?.length ?? 0 },
      { stacks: rows[2]?.length ?? 0, scale: LAND_SCALE },
    ])
  }

  it('falls under 64 px with every card alone and gets back over it by merging the others', () => {
    const box = { width: 640, height: 280 }
    const alone = fitSeat(false, box)
    const merged = fitSeat(true, box)
    expect(alone.cardWidth).toBeLessThan(GROUP_OTHERS_BELOW)
    expect(merged.cardWidth).toBeGreaterThanOrEqual(GROUP_OTHERS_BELOW)
    expect(merged.cardWidth).toBeGreaterThan(alone.cardWidth)
    expect(merged.scrolls).toBe(false)
  })

  it('never makes the cards smaller, in any box from 280x120 to 1700x520', () => {
    for (const width of [280, 400, 640, 900, 1200, 1700]) {
      for (const height of [120, 200, 300, 400, 520]) {
        const box = { width, height }
        expect(fitSeat(true, box).cardWidth, `${width}x${height}`).toBeGreaterThanOrEqual(
          fitSeat(false, box).cardWidth,
        )
      }
    }
    expect(FIT_LIMITS.minWidth).toBeLessThan(GROUP_OTHERS_BELOW)
  })
})
