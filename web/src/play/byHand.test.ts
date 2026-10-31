// Goal: a seat owes the table its by-hand clauses from the resolution of a spell or ability of its own with `ObjectView.manual` until it passes priority, in the order the events came, as the server holds the seat; other seats' objects, objects without clauses, anything that did not resolve, hidden events and a viewer with no seat owe nothing, and an ability put on the stack and resolved inside one batch has the clauses of its source card.
import { describe, expect, it } from 'vitest'
import { buildFixture } from '@/features/table/dev/fixture'
import { must } from '@/test-utils/batches'
import type { Event, EventView, ObjectId, ObjectView } from '@/protocol/generated'
import { owesByHand } from './byHand'
import { indexById } from './view'

const view = buildFixture({ seats: 2 }).view
const spells = view.objects.filter((object) => object.zone === 'Stack')
const mine = must(spells.find((object) => object.controller === 0))
const theirs = must(spells.find((object) => object.controller === 1))
const CLAUSE = { text: 'Flip a coin.', reason: 'random' } as const
const withClauses = (object: ObjectView): ObjectView => ({ ...object, manual: [CLAUSE] })

const table = (...objects: ObjectView[]) => indexById(objects)
const NONE = table()
const pub = (event: Event): EventView => ({ Public: event })
const spellResolved = (object: ObjectId): EventView => pub({ SpellResolved: { object } })
const ended = (object: ObjectId, outcome: 'Resolved' | 'Fizzled' | 'Countered' | 'Lapsed'): EventView =>
  pub({ ResolutionEnded: { object, outcome } })
const passed = (player: number): EventView => pub({ PriorityPassed: { player } })

describe('owesByHand', () => {
  it.each<[string, boolean, EventView[], number | null, boolean]>([
    ['its own spell with clauses resolves', false, [spellResolved(mine.id)], 0, true],
    ['its own ability with clauses resolves', false, [ended(mine.id, 'Resolved')], 0, true],
    [
      'it owed already and nothing relevant happens',
      true,
      [pub({ PermanentTapped: { object: 1 } })],
      0,
      true,
    ],
    ['it passes priority', true, [passed(0)], 0, false],
    ['another seat passes priority', true, [passed(1)], 0, true],
    [
      'it passes, then its spell resolves in the same batch',
      false,
      [passed(0), spellResolved(mine.id)],
      0,
      true,
    ],
    [
      'its spell resolves, then it passes in the same batch',
      false,
      [spellResolved(mine.id), passed(0)],
      0,
      false,
    ],
    [
      "a seat that is not the viewer's resolves its spell with clauses",
      false,
      [spellResolved(theirs.id)],
      0,
      false,
    ],
    ['the viewer has no seat', true, [spellResolved(mine.id)], null, false],
    ['a resolution of an object the view does not know', false, [spellResolved(9999)], 0, false],
    ['an ability that was countered', false, [ended(mine.id, 'Countered')], 0, false],
    ['an ability that fizzled', false, [ended(mine.id, 'Fizzled')], 0, false],
    ['an ability that lapsed', false, [ended(mine.id, 'Lapsed')], 0, false],
    ['a hidden event', false, [{ Hidden: { ManualMove: { player: 0, to: 'Graveyard' } } }], 0, false],
  ])('when %s', (_what, owed, events, seat, expected) => {
    const before = table(withClauses(mine), withClauses(theirs))
    expect(owesByHand(owed, events, seat, before, NONE)).toBe(expected)
  })

  it('owes nothing for a spell without clauses, with the clauses empty or left out', () => {
    for (const object of [mine, { ...mine, manual: [] }]) {
      expect(owesByHand(false, [spellResolved(mine.id)], 0, table(object), NONE)).toBe(false)
    }
  })

  it('reads an object that is only in the view after the batch (a permanent that entered)', () => {
    expect(owesByHand(false, [spellResolved(mine.id)], 0, NONE, table(withClauses(mine)))).toBe(true)
  })

  it('reads the view before the batch first: the clauses it held when the object resolved', () => {
    expect(owesByHand(false, [spellResolved(mine.id)], 0, table(mine), table(withClauses(mine)))).toBe(false)
  })
})

describe('an ability put on the stack and resolved inside one batch', () => {
  // The fields of the event the rule does not read stay out of the fixture.
  const put = (object: ObjectId, source: ObjectId, controller: number): EventView =>
    pub({ AbilityPutOnStack: { object, source: { id: source, counter: 0 }, controller } as never })
  const ABILITY = 9001
  const goes = (source: ObjectId, controller: number, ...more: EventView[]): EventView[] => [
    put(ABILITY, source, controller),
    ...more,
  ]

  it.each<[string, EventView[], ObjectView, boolean]>([
    [
      'its own, from a card with clauses',
      goes(mine.id, 0, ended(ABILITY, 'Resolved')),
      withClauses(mine),
      true,
    ],
    ['its own, from a card without clauses', goes(mine.id, 0, ended(ABILITY, 'Resolved')), mine, false],
    [
      "another seat's, from a card with clauses",
      goes(theirs.id, 1, ended(ABILITY, 'Resolved')),
      withClauses(theirs),
      false,
    ],
    ['its own that is countered', goes(mine.id, 0, ended(ABILITY, 'Countered')), withClauses(mine), false],
    [
      'its own, then the seat passes',
      goes(mine.id, 0, ended(ABILITY, 'Resolved'), passed(0)),
      withClauses(mine),
      false,
    ],
    [
      'its own, after the seat and another passed',
      goes(mine.id, 0, passed(0), passed(1), ended(ABILITY, 'Resolved')),
      withClauses(mine),
      true,
    ],
  ])('%s', (_what, events, source, expected) => {
    expect(owesByHand(false, events, 0, NONE, table(source))).toBe(expected)
  })

  it('knows nothing of an ability whose source the view never held', () => {
    expect(owesByHand(false, goes(mine.id, 0, ended(ABILITY, 'Resolved')), 0, NONE, NONE)).toBe(false)
  })
})
