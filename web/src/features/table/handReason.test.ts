// Goal: a hand card the priority offer leaves out gets a note with the server's reason when the offer names one, else the reason the table can tell from public state (sorcery speed outside the viewer's main phase or with a spell on the stack, a land after the land of the turn, a spell that costs more than every source and the pool could make) and "not now" otherwise, an offered card gets none, and every reason has a label and a line.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { ME, miniView, momentOf, MY_FOREST, MY_HAND_CARD } from '@/test-utils/miniTable'
import type { ManaCost, ObjectView, PriorityOptions, StateView } from '@/protocol/generated'
import { handNotes, manaAtHand, REASON_WORDS, unplayableReason } from './handReason'
import type { HandReason, Unplayable } from './handReason'

const NONE: PriorityOptions = {
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: false,
  can_pass: true,
}

/** Mana abilities of `sources` permanents, the first with a second ability that makes two. */
const sources = (count: number): PriorityOptions => ({
  ...NONE,
  mana_abilities: Array.from({ length: count }, (_, index) => ({
    source: { object: 100 + index, ability: 0 },
    produces: 'Green' as const,
  })),
})

const generic = (amount: number): ManaCost => [{ Generic: amount }]

interface Table {
  own: boolean
  step: 'Main1' | 'Main2' | 'Upkeep' | 'DeclareAttackers'
  stack: number
  lands: number
}

const MAIN: Table = { own: true, step: 'Main1', stack: 0, lands: 0 }

function viewOf(table: Table): StateView {
  const view = miniView()
  view.turn = { ...view.turn, active: table.own ? ME : 1, step: table.step, lands_played: table.lands }
  view.stack = { count: table.stack, cards: null }
  return view
}

function cardOf(view: StateView, types: ObjectView['types'], over: Partial<ObjectView> = {}): ObjectView {
  const card = must(view.objects.find((object) => object.id === MY_HAND_CARD))
  return { ...card, types, keywords: [], mana_cost: generic(1), ...over }
}

describe('unplayableReason', () => {
  it.each<[string, Table, ObjectView['types'], Partial<ObjectView>, PriorityOptions, HandReason | null]>([
    ['an offered spell', MAIN, ['Creature'], {}, { ...NONE, castable: [MY_HAND_CARD] }, null],
    ['an offered land', MAIN, ['Land'], {}, { ...NONE, playable_lands: [MY_HAND_CARD] }, null],
    [
      'an offered face',
      MAIN,
      ['Creature'],
      {},
      { ...NONE, faces: [{ card: MY_HAND_CARD, face: 1, name: 'Back', land: false }] },
      null,
    ],
    ['a creature on the opponent’s turn', { ...MAIN, own: false }, ['Creature'], {}, sources(5), 'Timing'],
    ['a sorcery in the viewer’s upkeep', { ...MAIN, step: 'Upkeep' }, ['Sorcery'], {}, sources(5), 'Timing'],
    ['a sorcery while a spell is on the stack', { ...MAIN, stack: 1 }, ['Sorcery'], {}, sources(5), 'Timing'],
    ['a land in combat', { ...MAIN, step: 'DeclareAttackers' }, ['Land'], {}, NONE, 'Timing'],
    [
      'a land after the land of the turn',
      { ...MAIN, step: 'Main2', lands: 1 },
      ['Land'],
      {},
      NONE,
      'LandPlayed',
    ],
    ['a land the offer leaves out for another reason', MAIN, ['Land'], {}, NONE, 'Unknown'],
    [
      'an instant that costs more than the mana at hand',
      { ...MAIN, own: false },
      ['Instant'],
      { mana_cost: generic(4) },
      sources(3),
      'Mana',
    ],
    [
      'a creature with Flash that costs more than the mana at hand',
      { ...MAIN, stack: 1 },
      ['Creature'],
      { keywords: ['Flash'], mana_cost: generic(2) },
      sources(1),
      'Mana',
    ],
    [
      'an instant the mana would pay for',
      { ...MAIN, own: false },
      ['Instant'],
      { mana_cost: generic(3) },
      sources(3),
      'Unknown',
    ],
    [
      'a sorcery in the main phase the mana would pay for',
      MAIN,
      ['Sorcery'],
      { mana_cost: generic(0) },
      NONE,
      'Unknown',
    ],
  ])('%s', (_name, table, types, over, options, reason) => {
    const view = viewOf(table)
    expect(unplayableReason(cardOf(view, types, over), view, options)).toBe(reason)
  })
})

describe('unplayableReason with the server reason', () => {
  const told = (reason: Unplayable): PriorityOptions => ({
    ...sources(5),
    unplayable: [{ card: MY_HAND_CARD, reason }],
  })
  it.each<Unplayable>(['Timing', 'LandPlayed', 'Mana', 'Targets', 'Cost', 'Restricted'])(
    '%s replaces the guess',
    (reason) => {
      // A sorcery in the viewer's main phase with mana to spare: the guess would say "Not now".
      const view = viewOf(MAIN)
      expect(unplayableReason(cardOf(view, ['Sorcery']), view, told(reason))).toBe(reason)
    },
  )

  it('names the reason in the note and its spoken words', () => {
    const view = viewOf(MAIN)
    const { objectsById } = momentOf(view)
    expect([...handNotes(view, objectsById, told('Targets'))]).toEqual([
      [MY_HAND_CARD, { hint: 'No targets', words: 'no targets' }],
    ])
  })

  it('guesses for a card the server does not name', () => {
    const view = viewOf({ ...MAIN, own: false })
    const options = { ...NONE, unplayable: [{ card: 999, reason: 'Cost' as const }] }
    expect(unplayableReason(cardOf(view, ['Creature']), view, options)).toBe('Timing')
  })
})

describe('manaAtHand', () => {
  it('counts the pool and one use of each source, at its largest', () => {
    const view = viewOf(MAIN)
    must(view.players[ME]).mana = { counts: [1, 0, 0, 0, 2, 0] }
    const options: PriorityOptions = {
      ...NONE,
      mana_abilities: [
        { source: { object: MY_FOREST, ability: 0 }, produces: 'Green' },
        { source: { object: MY_FOREST, ability: 1 }, produces: 'Green', count: 2 },
        { source: { object: 200, ability: 0 }, produces: 'Red' },
      ],
    }
    expect(manaAtHand(view, options)).toBe(3 + 2 + 1)
  })
})

describe('handNotes', () => {
  it('notes each hand card the offer leaves out, and only those', () => {
    const view = viewOf({ ...MAIN, own: false })
    const { objectsById } = momentOf(view)
    expect([...handNotes(view, objectsById, NONE)]).toEqual([
      [MY_HAND_CARD, { hint: REASON_WORDS.Timing.label, words: 'sorcery speed' }],
    ])
    expect(handNotes(view, objectsById, { ...NONE, castable: [MY_HAND_CARD] }).size).toBe(0)
  })
})

describe('REASON_WORDS', () => {
  it.each(Object.entries(REASON_WORDS))('%s has a short label and a line', (_reason, words) => {
    expect(words.label.length).toBeGreaterThan(0)
    expect(words.label.length).toBeLessThanOrEqual(14)
    expect(words.line.length).toBeGreaterThan(words.label.length)
  })
})
