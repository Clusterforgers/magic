// Goal: every moment of play (cast, activate, trigger, resolve, counter, fizzle, play a land, draw, a move a resolution makes) becomes the moves and marks the design names: from where it was to where it goes, with the stack in between, a card back for what the viewer may not see, the spotlight for another seat's spell or one that passes through in a batch, marks that name the spell that caused them, a cap on the moves of one batch, and untouched inputs.
import { describe, expect, it } from 'vitest'
import { deepFreeze } from '@/test-utils/batches'
import {
  afterChange,
  FOE,
  FOE_BEAR,
  FOE_IDOL,
  FOE_SPELL,
  ME,
  miniView,
  momentOf,
  MY_BEAR,
  MY_HAND_CARD,
  pub,
} from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import type { Event, EventView, ObjectView, Snapshot, Target, Zone } from '@/protocol/generated'
import type { Moment } from './derive'
import { anchorOf, deriveMoves, MAX_MOVES, placeOf } from './moves'
import type { Moves } from './moves'

const base = momentOf(miniView())

const COUNTER = 41
const UNSEEN = 40
const ABILITY = 50

const at = (id: number): Target => ({ Object: { id, counter: 0 } })

const moveTo =
  (id: number, zone: Zone, extra: Partial<ObjectView> = {}) =>
  (draft: Draft): void => {
    draft.edit(id, (object) => {
      Object.assign(object, { zone, permanent: zone === 'Battlefield' ? object.permanent : null, ...extra })
    })
  }

/** A copy of the viewer's Bear as a new object in `zone`. */
const extra =
  (id: number, zone: Zone, over: Partial<ObjectView> = {}) =>
  (draft: Draft): void => {
    const bear = draft.view.objects.find((object) => object.id === MY_BEAR)
    if (bear === undefined) throw new Error('no bear')
    draft.add({ ...structuredClone(bear), id, zone, permanent: null, ...over })
  }

const all =
  (...changes: ((draft: Draft) => void)[]) =>
  (draft: Draft): void => {
    for (const change of changes) change(draft)
  }

function snapshot(id: number, zone: Zone, controller: number): Snapshot {
  return {
    id,
    counter: 0,
    controller,
    owner: controller,
    zone,
    counters: [],
    attachments: [],
  } as unknown as Snapshot
}

const putOnStack = (source: number, controller: number, triggered = false): Event => ({
  AbilityPutOnStack: {
    object: ABILITY,
    source: { id: source, counter: 0 },
    snapshot: snapshot(source, 'Battlefield', controller),
    ability: 0,
    controller,
    ...(triggered ? { cause: { subjects: [], snapshots: [] } } : {}),
  },
})

const began = (object: number): Event => ({ ResolutionBegan: { object } })
const ended = (object: number, outcome: 'Resolved' | 'Countered' | 'Fizzled' | 'Lapsed'): Event => ({
  ResolutionEnded: { object, outcome },
})

function movesOf(change: (draft: Draft) => void, events: EventView[], from: Moment = base): Moves {
  const after = afterChange(from, change, events)
  deepFreeze(from)
  deepFreeze(after)
  deepFreeze(events)
  return deriveMoves(from, after, events)
}

/** The moves as `kind from > to`, with `*` for the spotlight. */
const shape = ({ moves }: Moves): string[] =>
  moves.map(
    (move) =>
      `${move.kind} ${anchorOf(move.from)} > ${anchorOf(move.to)}${move.spotlight ? ' *' : ''}${move.look === null ? ' back' : ''}`,
  )

const marked = ({ marks }: Moves): string[] =>
  marks.map((mark) => `${mark.kind} ${mark.object}${mark.by === undefined ? '' : ` by ${mark.by}`}`)

const onStackWith = (targets: Target[][]) => moveTo(MY_HAND_CARD, 'Stack', { targets })

/** Each moment: what changes, the events, then the moves and the marks it must give. */
const MOMENTS: [string, (draft: Draft) => void, EventView[], string[], string[]][] = [
  [
    'the viewer casts a spell from the hand at a creature',
    onStackWith([[at(FOE_BEAR)]]),
    pub({ SpellCast: { player: ME, object: MY_HAND_CARD } }),
    ['cast 0:hand > stack'],
    [`targeted ${FOE_BEAR} by ${MY_HAND_CARD}`],
  ],
  [
    'the opponent casts a card the viewer never saw',
    extra(UNSEEN, 'Stack', { owner: FOE, controller: FOE }),
    pub({ SpellCast: { player: FOE, object: UNSEEN } }),
    ['cast 1:hand > stack *'],
    [],
  ],
  [
    'a spell cast and resolved in one batch stands in the spotlight and goes on to the battlefield',
    moveTo(MY_HAND_CARD, 'Battlefield', { permanent: { tapped: false, damage: 0, summoning_sick: true } }),
    pub(
      { SpellCast: { player: ME, object: MY_HAND_CARD } },
      began(MY_HAND_CARD),
      { SpellResolved: { object: MY_HAND_CARD } },
      ended(MY_HAND_CARD, 'Resolved'),
    ),
    ['cast 0:hand > stack *', 'resolved stack > 0:battlefield'],
    [`resolved ${MY_HAND_CARD}`],
  ],
  [
    'a spell on the stack resolves into a permanent',
    moveTo(FOE_SPELL, 'Battlefield', { permanent: { tapped: false, damage: 0, summoning_sick: true } }),
    pub(began(FOE_SPELL), { SpellResolved: { object: FOE_SPELL } }, ended(FOE_SPELL, 'Resolved')),
    ['resolved stack > 1:battlefield'],
    [`resolved ${FOE_SPELL}`],
  ],
  [
    'a counterspell counters the spell under it, which goes to its owner’s graveyard',
    all(extra(COUNTER, { Graveyard: ME }), moveTo(FOE_SPELL, { Graveyard: FOE })),
    pub(began(COUNTER), began(FOE_SPELL), ended(FOE_SPELL, 'Countered'), ended(COUNTER, 'Resolved')),
    ['countered stack > 1:graveyard', 'resolved stack > 0:graveyard'],
    [`countered ${FOE_SPELL} by ${COUNTER}`, `resolved ${COUNTER}`],
  ],
  [
    'a spell whose targets are gone fizzles',
    moveTo(FOE_SPELL, { Graveyard: FOE }),
    pub(began(FOE_SPELL), ended(FOE_SPELL, 'Fizzled')),
    ['fizzled stack > 1:graveyard'],
    [`fizzled ${FOE_SPELL}`],
  ],
  [
    'an activated ability flies from its permanent',
    extra(ABILITY, 'Stack', { kind: 'Ability', source: MY_BEAR, targets: [[at(FOE_BEAR)]] }),
    pub(
      { AbilityActivated: { player: ME, source: { id: MY_BEAR, counter: 0 }, ability: 0 } },
      putOnStack(MY_BEAR, ME),
    ),
    ['activated 0:battlefield > stack'],
    [`activated ${MY_BEAR}`, `targeted ${FOE_BEAR} by ${ABILITY}`],
  ],
  [
    'a triggered ability flies from its permanent',
    extra(ABILITY, 'Stack', { kind: 'Ability', source: FOE_BEAR, owner: FOE, controller: FOE }),
    pub(putOnStack(FOE_BEAR, FOE, true)),
    ['triggered 1:battlefield > stack'],
    [`triggered ${FOE_BEAR}`],
  ],
  [
    'an ability that resolves ends on the stack',
    all(extra(ABILITY, 'Stack', { kind: 'Ability', source: MY_BEAR }), (draft) => draft.leave(ABILITY)),
    pub(putOnStack(MY_BEAR, ME), began(ABILITY), ended(ABILITY, 'Resolved')),
    ['activated 0:battlefield > stack'],
    [`activated ${MY_BEAR}`, `resolved ${ABILITY}`],
  ],
  [
    'the viewer plays a land',
    moveTo(MY_HAND_CARD, 'Battlefield', { permanent: { tapped: false, damage: 0, summoning_sick: false } }),
    pub({ LandPlayed: { player: ME, object: MY_HAND_CARD } }),
    ['landed 0:hand > 0:battlefield'],
    [],
  ],
  [
    'the viewer draws a card and the opponent draws one unseen',
    extra(60, { Hand: ME }),
    [{ Public: { CardDrawn: { player: ME, object: 60 } } }, { Hidden: { CardDrawn: { player: FOE } } }],
    ['drawn 0:library > 0:hand', 'drawn 1:library > 1:hand back'],
    ['drawn 60'],
  ],
  [
    'a resolving spell exiles a creature and returns another to its owner’s hand',
    all(moveTo(FOE_BEAR, 'Exile'), moveTo(MY_BEAR, { Hand: ME })),
    pub(
      began(FOE_SPELL),
      { ZoneChanged: { object: FOE_BEAR, from: 'Battlefield', to: 'Exile', cause: 'Exiled' } },
      { ZoneChanged: { object: MY_BEAR, from: 'Battlefield', to: { Hand: ME }, cause: 'Moved' } },
    ),
    ['moved 1:battlefield > 1:exile', 'moved 0:battlefield > 0:hand'],
    [`exiled ${FOE_BEAR} by ${FOE_SPELL}`, `bounced ${MY_BEAR} by ${FOE_SPELL}`],
  ],
  [
    'a resolving spell kills a creature and destroys a permanent that is no creature',
    all(moveTo(FOE_BEAR, { Graveyard: FOE }), moveTo(FOE_IDOL, { Graveyard: FOE })),
    pub(
      began(FOE_SPELL),
      { CreatureDied: { object: FOE_BEAR, cause: 'LethalDamage' } },
      { ZoneChanged: { object: FOE_BEAR, from: 'Battlefield', to: { Graveyard: FOE }, cause: 'Moved' } },
      { ZoneChanged: { object: FOE_IDOL, from: 'Battlefield', to: { Graveyard: FOE }, cause: 'Moved' } },
    ),
    ['moved 1:battlefield > 1:graveyard', 'moved 1:battlefield > 1:graveyard'],
    [
      `died ${FOE_BEAR} by ${FOE_SPELL}`,
      `died ${FOE_BEAR} by ${FOE_SPELL}`,
      `died ${FOE_IDOL} by ${FOE_SPELL}`,
    ],
  ],
  [
    'a move onto or off the stack is the resolution’s, not a move of its own',
    moveTo(FOE_SPELL, { Graveyard: FOE }),
    pub({ ZoneChanged: { object: FOE_SPELL, from: 'Stack', to: { Graveyard: FOE }, cause: 'Countered' } }),
    [],
    [],
  ],
  ['a batch with nothing played moves nothing', () => {}, pub({ PriorityPassed: { player: ME } }), [], []],
]

describe('deriveMoves', () => {
  it.each(MOMENTS)('%s', (_name, change, events, moves, marks) => {
    const result = movesOf(change, events)
    expect(shape(result)).toEqual(moves)
    expect(marked(result)).toEqual(marks)
  })

  it('names the card it flies, and the places it leaves and reaches', () => {
    const { moves } = movesOf(
      onStackWith([[at(FOE_BEAR)], [{ Player: FOE }]]),
      pub({ SpellCast: { player: ME, object: MY_HAND_CARD } }),
    )
    expect(moves).toEqual([
      expect.objectContaining({
        object: MY_HAND_CARD,
        look: expect.objectContaining({ id: MY_HAND_CARD, zone: 'Stack' }),
        from: { seat: ME, zone: 'hand', object: MY_HAND_CARD },
        to: { stack: true, object: MY_HAND_CARD },
        targets: [at(FOE_BEAR), { Player: FOE }],
        order: 0,
      }),
    ])
  })

  it(`flies at most ${MAX_MOVES} moves a batch, in event order`, () => {
    const draws: EventView[] = Array.from({ length: 12 }, () => ({ Hidden: { CardDrawn: { player: FOE } } }))
    const { moves } = movesOf(() => {}, draws)
    expect(moves.map((move) => move.order)).toEqual([...Array(MAX_MOVES).keys()])
  })
})

describe('placeOf', () => {
  it.each<[Zone, string]>([
    ['Stack', 'stack'],
    ['Battlefield', '1:battlefield'],
    ['PhasedOut', '1:battlefield'],
    ['Exile', '1:exile'],
    [{ Hand: 0 }, '0:hand'],
    [{ Library: 2 }, '2:library'],
    [{ Graveyard: 3 }, '3:graveyard'],
    [{ Command: 0 }, '0:command'],
  ])('%j is anchored at %s', (zone, anchor) => {
    expect(anchorOf(placeOf(zone, 1))).toBe(anchor)
    expect(placeOf(zone, 1, 7)).toMatchObject({ object: 7 })
  })
})
