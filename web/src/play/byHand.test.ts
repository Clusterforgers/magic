// Goal: a seat owes the table its by-hand clauses from the resolution of a spell or ability of its own whose object the server flags `manual_owed` until it passes priority, in the order the events came, as the server holds the seat, and the hold names the card; the flag alone decides (clauses the server does not call owed, such as activated and loyalty abilities, hold nothing), other seats' objects, unflagged objects, anything that did not resolve, hidden events and a viewer with no seat owe nothing, and an ability put on the stack and resolved inside one batch has the flag of its source's card.
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
/** The server's word for an object whose card is owed at resolution. */
const withClauses = (object: ObjectView): ObjectView => ({ ...object, manual: [CLAUSE], manual_owed: true })

const table = (...objects: ObjectView[]) => indexById(objects)
const NONE = table()
const pub = (event: Event): EventView => ({ Public: event })
const spellResolved = (object: ObjectId): EventView => pub({ SpellResolved: { object } })
const ended = (object: ObjectId, outcome: 'Resolved' | 'Fizzled' | 'Countered' | 'Lapsed'): EventView =>
  pub({ ResolutionEnded: { object, outcome } })
const passed = (player: number): EventView => pub({ PriorityPassed: { player } })
const EARLIER = 'Earlier card'

describe('owesByHand', () => {
  it.each<[string, string | null, EventView[], number | null, string | null]>([
    ['its own spell with clauses resolves', null, [spellResolved(mine.id)], 0, mine.name],
    ['its own ability with clauses resolves', null, [ended(mine.id, 'Resolved')], 0, mine.name],
    [
      'it owed already and nothing relevant happens',
      EARLIER,
      [pub({ PermanentTapped: { object: 1 } })],
      0,
      EARLIER,
    ],
    ['it passes priority', EARLIER, [passed(0)], 0, null],
    ['another seat passes priority', EARLIER, [passed(1)], 0, EARLIER],
    ['a second card resolves while it owes the first', EARLIER, [spellResolved(mine.id)], 0, mine.name],
    [
      'it passes, then its spell resolves in the same batch',
      null,
      [passed(0), spellResolved(mine.id)],
      0,
      mine.name,
    ],
    [
      'its spell resolves, then it passes in the same batch',
      null,
      [spellResolved(mine.id), passed(0)],
      0,
      null,
    ],
    [
      "a seat that is not the viewer's resolves its spell with clauses",
      null,
      [spellResolved(theirs.id)],
      0,
      null,
    ],
    ['the viewer has no seat', EARLIER, [spellResolved(mine.id)], null, null],
    ['a resolution of an object the view does not know', null, [spellResolved(9999)], 0, null],
    ['an ability that was countered', null, [ended(mine.id, 'Countered')], 0, null],
    ['an ability that fizzled', null, [ended(mine.id, 'Fizzled')], 0, null],
    ['an ability that lapsed', null, [ended(mine.id, 'Lapsed')], 0, null],
    ['a hidden event', null, [{ Hidden: { ManualMove: { player: 0, to: 'Graveyard' } } }], 0, null],
  ])('when %s', (_what, owed, events, seat, expected) => {
    const before = table(withClauses(mine), withClauses(theirs))
    expect(owesByHand(owed, events, seat, before, NONE)).toBe(expected)
  })

  it.each<[string, Partial<ObjectView>]>([
    ['no flag and no clauses', {}],
    ['no flag and no clauses, the clauses empty', { manual: [] }],
    ['the flag false with an owed-looking clause', { manual: [CLAUSE], manual_owed: false }],
    ['a mana ability left to the table', { manual: [{ text: '{T}: Add {C}.', reason: 'by hand' }] }],
    [
      'an activated ability left to the table',
      { manual: [{ text: '{2}, {T}: Draw a card.', reason: 'by hand' }] },
    ],
    [
      'loyalty abilities left to the table',
      { manual: [{ text: '+1: Draw a card.\n−2: Destroy target creature.', reason: 'by hand' }] },
    ],
  ])('owes nothing for a resolved spell with %s', (_what, fields) => {
    const object: ObjectView = { ...mine, manual: undefined, manual_owed: undefined, ...fields }
    expect(owesByHand(null, [spellResolved(mine.id)], 0, table(object), NONE)).toBeNull()
    expect(owesByHand(null, [spellResolved(mine.id)], 0, NONE, table(object))).toBeNull()
  })

  it('trusts the flag, not the clauses: a flagged object holds the seat although none are listed', () => {
    const object: ObjectView = { ...mine, manual: undefined, manual_owed: true }
    expect(owesByHand(null, [spellResolved(mine.id)], 0, table(object), NONE)).toBe(mine.name)
  })

  it('reads an object that is only in the view after the batch (a permanent that entered)', () => {
    expect(owesByHand(null, [spellResolved(mine.id)], 0, NONE, table(withClauses(mine)))).toBe(mine.name)
  })

  it('reads the view before the batch first: the flag it held when the object resolved', () => {
    expect(owesByHand(null, [spellResolved(mine.id)], 0, table(mine), table(withClauses(mine)))).toBeNull()
  })

  it('reads an ability that was put on the stack in an earlier batch from its own view', () => {
    const ability: ObjectView = { ...withClauses(mine), id: 9002, kind: 'Ability', manual: undefined }
    expect(owesByHand(null, [ended(ability.id, 'Resolved')], 0, table(ability), NONE)).toBe(mine.name)
    const plain: ObjectView = { ...ability, manual_owed: undefined }
    expect(owesByHand(null, [ended(plain.id, 'Resolved')], 0, table(plain), NONE)).toBeNull()
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

  it.each<[string, EventView[], ObjectView, string | null]>([
    [
      'its own, from a card with clauses',
      goes(mine.id, 0, ended(ABILITY, 'Resolved')),
      withClauses(mine),
      mine.name,
    ],
    ['its own, from a card without clauses', goes(mine.id, 0, ended(ABILITY, 'Resolved')), mine, null],
    [
      'its own, from a card whose clauses are not owed',
      goes(mine.id, 0, ended(ABILITY, 'Resolved')),
      { ...mine, manual: [CLAUSE], manual_owed: false },
      null,
    ],
    [
      "another seat's, from a card with clauses",
      goes(theirs.id, 1, ended(ABILITY, 'Resolved')),
      withClauses(theirs),
      null,
    ],
    ['its own that is countered', goes(mine.id, 0, ended(ABILITY, 'Countered')), withClauses(mine), null],
    [
      'its own, then the seat passes',
      goes(mine.id, 0, ended(ABILITY, 'Resolved'), passed(0)),
      withClauses(mine),
      null,
    ],
    [
      'its own, after the seat and another passed',
      goes(mine.id, 0, passed(0), passed(1), ended(ABILITY, 'Resolved')),
      withClauses(mine),
      mine.name,
    ],
  ])('%s', (_what, events, source, expected) => {
    expect(owesByHand(null, events, 0, NONE, table(source))).toBe(expected)
  })

  it('knows nothing of an ability whose source the view never held', () => {
    expect(owesByHand(null, goes(mine.id, 0, ended(ABILITY, 'Resolved')), 0, NONE, NONE)).toBeNull()
  })
})
