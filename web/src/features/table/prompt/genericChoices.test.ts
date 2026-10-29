/** Goal: for each decision kind and its edge cases (nothing to choose, a minimum above one, a left-out answer, a long list, a decision that cannot be backed out of), the generic answers list exactly the answers the options allow, in order, with the cancel of a cast last, and each answer is the wire answer of its kind. */
import { describe, expect, it } from 'vitest'
import type { DecisionKind, ObjectId, Target } from '@/protocol/generated'
import { askerOf, genericChoices, MOST_ANSWERS, namedObjects } from './genericChoices'
import type { GenericWords } from './genericChoices'

const words: GenericWords = {
  object: (id: ObjectId) => `Card ${id}`,
  seat: (seat) => `Seat ${seat}`,
  viewer: 0,
  cancel: 'Cancel the spell',
}
const object = (id: number): Target => ({ Object: { id, counter: 0 } })
const labels = (kind: DecisionKind): string[] =>
  genericChoices(kind, words).choices.map((choice) => choice.label)
const mode = (index: number) => ({ index, label: `Mode ${index}` })
const targets = (given: Partial<Extract<DecisionKind, { ChooseTargets: unknown }>['ChooseTargets']>) =>
  ({
    ChooseTargets: {
      source: 1,
      spec: 0,
      legal: [{ Player: 0 }, object(7)],
      min: 1,
      max: 1,
      can_cancel: false,
      ...given,
    },
  }) as DecisionKind

describe('the labels of each kind', () => {
  it.each<[string, DecisionKind, string[]]>([
    [
      'a mulligan that cannot be taken',
      {
        Mulligan: {
          hand: [],
          mulligans_taken: 0,
          can_mulligan: false,
          free_mulligans: 0,
          bottom_if_keep: 0,
          bottom_if_mulligan: 0,
        },
      },
      ['Keep this hand'],
    ],
    ['no cards to put back', { BottomCards: { hand: [], count: 1 } }, []],
    [
      'one card to put back',
      { BottomCards: { hand: [4, 5], count: 1 } },
      ['Put the first 1 card on the bottom'],
    ],
    ['an empty hand to discard from', { Discard: { hand: [], count: 1 } }, []],
    [
      'a payment with neither way',
      {
        PayMana: {
          spell: 1,
          cost: [],
          pool: { counts: [0, 0, 0, 0, 0, 0] },
          sources: [],
          auto_available: false,
          can_cancel: false,
          life: 20,
          max_life: 0,
        },
      },
      [],
    ],
    [
      'a payment the engine can make',
      {
        PayMana: {
          spell: 1,
          cost: [],
          pool: { counts: [0, 0, 0, 0, 0, 0] },
          sources: [],
          auto_available: true,
          can_cancel: false,
          life: 20,
          max_life: 0,
        },
      },
      ['Pay automatically'],
    ],
    ['a yes-or-no with only no', { YesNo: { source: 1, only_no: true } }, ['No']],
    ['targets, a player and a card', targets({}), ['Seat 0 (you)', 'Card 7']],
    ['targets with the same one twice', targets({ legal: [{ Player: 1 }, { Player: 1 }] }), ['Seat 1']],
    [
      'targets from which none may be left',
      targets({ min: 0, max: 2 }),
      ['Seat 0 (you)', 'Card 7', 'Choose no target'],
    ],
    ['two targets required', targets({ min: 2, max: 2, can_cancel: true }), ['Cancel the spell']],
    [
      'a triggered ability that cannot be backed out of',
      targets({ triggered: true }),
      ['Seat 0 (you)', 'Card 7', 'Remove the ability'],
    ],
    ['a cast that can', targets({ can_cancel: true }), ['Seat 0 (you)', 'Card 7', 'Cancel the spell']],
    [
      'one mode of two',
      {
        ChooseMode: {
          source: 1,
          modes: [mode(0), mode(1)],
          min: 1,
          max: 1,
          repeat: false,
          can_cancel: false,
        },
      },
      ['Mode 0', 'Mode 1'],
    ],
    [
      'two modes of three',
      {
        ChooseMode: {
          source: 1,
          modes: [mode(0), mode(1), mode(2)],
          min: 2,
          max: 2,
          repeat: false,
          can_cancel: true,
        },
      },
      ['Choose the first 2 modes', 'Cancel the spell'],
    ],
    [
      'up to two modes',
      {
        ChooseMode: {
          source: 1,
          modes: [mode(0), mode(1)],
          min: 0,
          max: 2,
          repeat: false,
          can_cancel: false,
        },
      },
      ['Mode 0', 'Mode 1'],
    ],
    [
      'a number past a left-out one',
      { ChooseNumber: { source: 1, min: 0, max: 4, except: [0, 1] } },
      ['2', '3', '4'],
    ],
    [
      'a number with no top',
      { ChooseNumber: { source: 1, min: 3, max: null, can_cancel: true } },
      [...Array.from({ length: MOST_ANSWERS }, (_, at) => String(3 + at)), 'Cancel the spell'],
    ],
    [
      'a division that cannot give each one',
      { DivideAmount: { source: 1, total: 1, among: [{ Player: 0 }, { Player: 1 }] } },
      [],
    ],
    [
      'a division with no one to give to',
      { DivideAmount: { source: 1, total: 3, among: [], can_cancel: true } },
      ['Cancel the spell'],
    ],
    [
      'cards, none allowed',
      { ChooseCards: { source: 1, from: [4, 5], min: 0, max: 2 } },
      ['Card 4', 'Card 5', 'Choose no card'],
    ],
    [
      'two cards required',
      { ChooseCards: { source: 1, from: [4, 5, 6], min: 2, max: 2 } },
      ['Choose the first 2 cards'],
    ],
    [
      'cards that can be backed out of',
      { ChooseCards: { source: 1, from: [4, 5, 6], min: 2, max: 3, can_cancel: true } },
      ['Choose the first 2 cards', 'Cancel the spell'],
    ],
    ['no card on offer', { ChooseCards: { source: 1, from: [], min: 1, max: 1 } }, []],
    [
      'players, a colour and a label',
      {
        ChooseOption: {
          source: 1,
          options: [{ Player: 0 }, { Player: 1 }, { Color: 'Green' }, { Label: 'Fire' }],
        },
      },
      ['Seat 0 (you)', 'Seat 1', 'Green', 'Fire'],
    ],
    [
      'optional costs that can be backed out of',
      {
        ChooseCosts: {
          source: 1,
          optional: [
            { index: 0, label: 'Kicker {R}' },
            { index: 65537, label: 'Gift to player 2' },
          ],
          can_cancel: true,
        },
      },
      ['Pay no optional cost', 'Kicker {R}', 'Gift to player 2', 'Cancel the spell'],
    ],
    ['no replacement effect', { ChooseReplacement: { candidates: [] } }, []],
  ])('%s', (_what, kind, expected) => {
    expect(labels(kind)).toEqual(expected)
  })
})

describe('the answers of each kind', () => {
  it.each<[string, DecisionKind, number, unknown]>([
    [
      'the second card of a list',
      { ChooseCards: { source: 1, from: [4, 5], min: 1, max: 1 } },
      1,
      { ChooseCards: [5] },
    ],
    ['no card', { ChooseCards: { source: 1, from: [4, 5], min: 0, max: 2 } }, 2, { ChooseCards: [] }],
    [
      'the first two cards',
      { ChooseCards: { source: 1, from: [4, 5, 6], min: 2, max: 2 } },
      0,
      { ChooseCards: [4, 5] },
    ],
    [
      'the second mode',
      {
        ChooseMode: {
          source: 1,
          modes: [mode(0), mode(3)],
          min: 1,
          max: 1,
          repeat: false,
          can_cancel: false,
        },
      },
      1,
      { ChooseMode: { Modes: [3] } },
    ],
    [
      'the first modes',
      {
        ChooseMode: {
          source: 1,
          modes: [mode(0), mode(1), mode(2)],
          min: 2,
          max: 2,
          repeat: false,
          can_cancel: false,
        },
      },
      0,
      { ChooseMode: { Modes: [0, 1] } },
    ],
    ['a player target', targets({}), 0, { ChooseTargets: { Targets: [{ Player: 0 }] } }],
    ['no target', targets({ min: 0 }), 2, { ChooseTargets: { Targets: [] } }],
    ['the trigger removed', targets({ triggered: true }), 2, { ChooseTargets: 'Remove' }],
    ['the cast taken back', targets({ can_cancel: true }), 2, { ChooseTargets: 'Cancel' }],
    ['a number', { ChooseNumber: { source: 1, min: 2, max: 5 } }, 1, { ChooseNumber: { Number: 3 } }],
    [
      'the number cast taken back',
      { ChooseNumber: { source: 1, min: 0, max: 1, can_cancel: true } },
      2,
      { ChooseNumber: 'Cancel' },
    ],
    [
      'an even division',
      { DivideAmount: { source: 1, total: 7, among: [{ Player: 0 }, { Player: 1 }, object(7)] } },
      0,
      {
        DivideAmount: {
          Division: [
            [{ Player: 0 }, 3],
            [{ Player: 1 }, 2],
            [object(7), 2],
          ],
        },
      },
    ],
    [
      'no optional cost',
      { ChooseCosts: { source: 1, optional: [{ index: 2, label: 'Buyback {3}' }] } },
      0,
      { ChooseCosts: { Costs: [] } },
    ],
    [
      'one optional cost',
      { ChooseCosts: { source: 1, optional: [{ index: 2, label: 'Buyback {3}' }] } },
      1,
      { ChooseCosts: { Costs: [2] } },
    ],
    [
      'the third option',
      { ChooseOption: { source: 1, options: [{ Label: 'a' }, { Label: 'b' }, { Label: 'c' }] } },
      2,
      { ChooseOption: 2 },
    ],
    [
      'a replacement effect',
      {
        ChooseReplacement: {
          candidates: [
            { source: 8, ability: 0 },
            { source: 9, ability: 1 },
          ],
        },
      },
      1,
      { ChooseReplacement: 1 },
    ],
    [
      'the triggers in order',
      {
        OrderTriggers: {
          triggers: [
            { source: { id: 1, counter: 0 }, ability: 0, when: 'a' },
            { source: { id: 2, counter: 0 }, ability: 0, when: 'b' },
            { source: { id: 3, counter: 0 }, ability: 0, when: 'c' },
          ],
        },
      },
      0,
      { OrderTriggers: [0, 1, 2] },
    ],
    [
      'lethal damage in order',
      { AssignDamage: { attacker: 1, power: 7, blockers: [8, 9], lethal: [3, 2] } },
      0,
      {
        AssignDamage: [
          [8, 3],
          [9, 4],
        ],
      },
    ],
    [
      'lethal damage with trample',
      { AssignDamage: { attacker: 1, power: 7, blockers: [8, 9], lethal: [3, 2], excess: { Player: 1 } } },
      0,
      {
        AssignDamage: [
          [8, 3],
          [9, 2],
        ],
      },
    ],
    [
      'all the damage to a lone blocker',
      { AssignDamage: { attacker: 1, power: 4, blockers: [8] } },
      0,
      { AssignDamage: [[8, 4]] },
    ],
  ])('%s', (_what, kind, press, answer) => {
    expect(genericChoices(kind, words).choices[press]?.answer).toEqual(answer)
  })

  it('marks only the cancel of a cast', () => {
    const { choices } = genericChoices(targets({ can_cancel: true }), words)
    expect(choices.map((choice) => choice.cancel === true)).toEqual([false, false, true])
  })
})

describe('a long list', () => {
  const many = Array.from({ length: 30 }, (_, at) => at + 100)
  it.each<[string, DecisionKind, number]>([
    ['cards', { ChooseCards: { source: 1, from: many, min: 1, max: 1 } }, 18],
    ['options', { ChooseOption: { source: 1, options: many.map((id) => ({ Label: `${id}` })) } }, 18],
    ['targets', targets({ legal: many.map(object) }), 18],
    ['no more than fits', { ChooseCards: { source: 1, from: many.slice(0, 12), min: 1, max: 1 } }, 0],
  ])('cuts %s to the first %i and counts the rest', (_what, kind, unlisted) => {
    const offer = genericChoices(kind, words)
    expect(offer.choices).toHaveLength(MOST_ANSWERS)
    expect(offer.unlisted).toBe(unlisted)
  })
})

describe('the objects a decision names', () => {
  it.each<[string, DecisionKind, ObjectId | undefined, ObjectId[]]>([
    ['targets', targets({}), 1, [7]],
    ['cards', { ChooseCards: { source: 3, from: [4, 5], min: 1, max: 1 } }, 3, [4, 5]],
    ['replacements', { ChooseReplacement: { candidates: [{ source: 8, ability: 0 }] } }, undefined, [8]],
    ['a yes-or-no', { YesNo: { source: 2 } }, 2, []],
    [
      'a pass',
      { Priority: { playable_lands: [], castable: [], mana_abilities: [], can_act: false, can_pass: true } },
      undefined,
      [],
    ],
  ])('%s', (_what, kind, asker, named) => {
    expect(askerOf(kind)).toBe(asker)
    expect(namedObjects(kind)).toEqual(named)
  })

  it('offers nothing for a kind it does not know', () => {
    const unknown = { Frobnicate: { source: 1 } } as unknown as DecisionKind
    expect(genericChoices(unknown, words)).toEqual({ choices: [], unlisted: 0 })
    expect(askerOf(unknown)).toBe(1)
  })
})
