/** Goal: the replacement prompt's sentence uses the engine's event words or a plain one, effects of one card are told apart by ability or as a lasting effect and still-equal names are numbered, and the keys are spelled for 0 to 12 effects. */
import { describe, expect, it } from 'vitest'
import type { ReplacementCandidate } from '@/protocol/generated'
import { replacementKeys, replacementLabels, replacementOrigin, replacementSentence } from './replacementText'

const effect = (source: number, ability = 0, shield?: number): ReplacementCandidate =>
  shield === undefined ? { source, ability } : { source, ability, shield }

describe('replacementSentence', () => {
  it.each([
    ['Create 2 token(s).', 'Create 2 token(s). Choose which applies first; the others apply after it.'],
    ['  Draw a card.  ', 'Draw a card. Choose which applies first; the others apply after it.'],
    [
      '',
      'Several replacement effects apply to one event. Choose which applies first; the others apply after it.',
    ],
  ])('reads "%s"', (text, sentence) => {
    expect(replacementSentence(text)).toBe(sentence)
  })
})

describe('replacementLabels', () => {
  it.each([
    ['one card each', [effect(1), effect(2)], ['Rest', 'Leyline'], ['Rest', 'Leyline']],
    ['a name the view lacks', [effect(1), effect(2)], ['Rest', undefined], ['Rest', 'A card']],
    [
      'two abilities of one card',
      [effect(1, 0), effect(1, 2)],
      ['Rest', 'Rest'],
      ['Rest (ability 1)', 'Rest (ability 3)'],
    ],
    [
      'an ability and a lasting effect',
      [effect(1, 0), effect(1, 0, 4)],
      ['Rest', 'Rest'],
      ['Rest (ability 1)', 'Rest (lasting effect)'],
    ],
    [
      'two lasting effects of one spell',
      [effect(1, 0, 4), effect(1, 0, 5)],
      ['Bolt', 'Bolt'],
      ['Bolt (lasting effect) 1', 'Bolt (lasting effect) 2'],
    ],
    ['two cards of one name', [effect(1), effect(2)], ['Rest', 'Rest'], ['Rest 1', 'Rest 2']],
    ['nothing', [], [], []],
  ])('names %s', (_what, candidates, names, expected) => {
    expect(replacementLabels(candidates, names)).toEqual(expected)
  })
})

describe('replacementOrigin', () => {
  it('says a spell or ability made a lasting effect and nothing for a card', () => {
    expect(replacementOrigin(effect(1, 0, 3))).toBe('A lasting effect made by a spell or ability')
    expect(replacementOrigin(effect(1))).toBeNull()
  })
})

describe('replacementKeys', () => {
  it.each([
    [0, 'Press 1 to choose it.'],
    [1, 'Press 1 to choose it.'],
    [2, 'Press 1 or 2 to choose.'],
    [9, 'Press 1 to 9 to choose.'],
    [10, 'Press 1 to 9, or 0, to choose.'],
    [12, 'Press 1 to 9, or 0, to choose one of the first ten.'],
  ])('for %i effects', (count, words) => {
    expect(replacementKeys(count)).toBe(words)
  })
})
