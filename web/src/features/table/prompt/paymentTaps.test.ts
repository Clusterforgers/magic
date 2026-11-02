/** Goal: tapping creatures for convoke (as mana of a colour they have, or as {1}), tapping artifacts for improvise and exiling graveyard cards for delve are read as the engine reads them: the colour meets a coloured symbol and spills into the generic part, improvise and delve pay generic mana only and never a colour or a source's own cost, nothing paid is wasted, each permanent or card is used once (a creature that is also a mana source is tapped once), the payment sends each kind in its own field, and a payment is offered only while it can be finished from what is left. */
import { describe, expect, it } from 'vitest'
import type { Color } from '@/protocol/generated'
import { cost, poolOf, source } from '@/test-utils/mana'
import { allowed, completable, covers, genericOf, owed, toPayment, usedBy } from './payment'
import type { Reach, Spend } from './payment'
import { costWords } from './payText'

const convoke = (creature: number, pays: Color | null = null): Spend => ({ kind: 'convoke', creature, pays })
const improvise = (artifact: number): Spend => ({ kind: 'improvise', artifact })
const delve = (card: number): Spend => ({ kind: 'delve', card })
const lands = (letters: string): Spend[] =>
  [...letters].map((letter, index) => ({ kind: 'tap', option: source(100 + index, letter) }))

describe('what generic-only payments cover', () => {
  it.each<[string, string, Spend[], string, boolean]>([
    ['convoke for {1} pays generic', '{2}', [convoke(1), convoke(2)], 'nothing', true],
    ['convoke for a colour meets the colour', '{G}', [convoke(1, 'Green')], 'nothing', true],
    ['convoke for a colour spills into generic', '{2}', [convoke(1, 'Green'), convoke(2)], 'nothing', true],
    ['convoke for {1} cannot meet a colour', '{G}', [convoke(1)], '1 green', false],
    ['improvise pays generic', '{3}', [improvise(1), improvise(2), improvise(3)], 'nothing', true],
    ['improvise cannot meet a colour', '{1}{G}', [improvise(1)], '1 green', false],
    ['improvise with the colour from a land', '{1}{G}', [improvise(1), ...lands('G')], 'nothing', true],
    ['delve pays generic', '{2}{B}', [delve(1), delve(2), ...lands('B')], 'nothing', true],
    ['delve cannot meet a colour', '{B}', [delve(1)], '1 black', false],
    [
      'three kinds together',
      '{4}{G}',
      [delve(1), improvise(2), convoke(3), convoke(4), convoke(5, 'Green')],
      'nothing',
      true,
    ],
    ['too much generic is waste', '{1}', [delve(1), delve(2)], '1 generic', false],
    ['still owed after some', '{4}', [delve(1), improvise(2)], '2 generic', false],
  ])('%s', (_name, text, spends, left, done) => {
    expect(costWords(owed(cost(text), spends))).toBe(left)
    expect(covers(cost(text), spends)).toBe(done)
  })
})

describe('using a piece once', () => {
  const none = poolOf('')
  const reach: Reach = { sources: [], maxLife: 0 }
  it.each<[string, Spend[], Spend, boolean]>([
    ['a creature once', [convoke(1)], convoke(1, 'Green'), false],
    ['another creature', [convoke(1)], convoke(2), true],
    ['a card of the graveyard once', [delve(1)], delve(1), false],
    ['another card', [delve(1)], delve(2), true],
    ['an artifact once', [improvise(1)], improvise(1), false],
    ['an artifact that is also tapped for mana', [improvise(100)], lands('G')[0] as Spend, false],
    ['a creature that is also tapped for mana', lands('G'), convoke(100), false],
    ['more than the cost asks', [delve(1), delve(2)], delve(3), false],
  ])('%s', (_name, spends, next, ok) => {
    expect(allowed(cost('{2}'), none, spends, next, [], reach)).toBe(ok)
  })

  it('knows what each piece uses up', () => {
    expect([convoke(1), improvise(1), delve(1)].map(usedBy)).toEqual(['o:1', 'o:1', 'd:1'])
    expect([convoke(1), convoke(2, 'Red'), improvise(3), delve(4)].map(genericOf)).toEqual([1, 0, 1, 1])
  })
})

describe('finishing a payment with what is left', () => {
  const none = poolOf('')
  const reach = (extras: Spend[]): Reach => ({ sources: [], maxLife: 0, extras })

  it.each<[string, string, Spend[], Spend[], boolean]>([
    ['delve can finish the generic part', '{2}', [], [delve(1), delve(2)], true],
    ['but not a colour that nothing else makes', '{2}{U}', [], [delve(1), delve(2), delve(3)], false],
    ['convoke in a colour can meet it', '{1}{U}', [], [convoke(1, 'Blue'), delve(1)], true],
    ['improvise cannot go over', '{1}', [improvise(1)], [improvise(2)], true],
    ['nothing left to finish with', '{3}', [], [delve(1)], false],
  ])('%s', (_name, text, spends, extras, ok) => {
    expect(completable(cost(text), none, spends, [], reach(extras))).toBe(ok)
  })

  it('does not search without end over many cards that pay alike', () => {
    const graveyard = Array.from({ length: 40 }, (_, at) => delve(at + 1))
    expect(completable(cost('{50}'), none, [], [], reach(graveyard))).toBe(false)
    expect(completable(cost('{20}'), none, [], [], reach(graveyard.slice(0, 20)))).toBe(true)
  })
})

describe('the payment on the wire', () => {
  it('sends each kind in its own field and leaves out the empty ones', () => {
    const spends = [convoke(1), convoke(2, 'Red'), improvise(3), delve(4), delve(5), ...lands('G')]
    expect(toPayment(spends)).toEqual({
      pool: [],
      taps: [{ object: 100, ability: 0 }],
      life: 0,
      convoke: [
        { creature: 1, pays: null },
        { creature: 2, pays: 'Red' },
      ],
      improvise: [3],
      delve: [4, 5],
    })
    expect(toPayment(lands('G'))).toEqual({ pool: [], taps: [{ object: 100, ability: 0 }], life: 0 })
  })
})
