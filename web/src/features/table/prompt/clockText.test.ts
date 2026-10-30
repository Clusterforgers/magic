// Goal: the countdown says what happens at zero for each kind of decision that has a clock, counts whole seconds up to the next one, and the sentence for a screen reader names the right number and the right consequence.
import { describe, expect, it } from 'vitest'
import type { DecisionKind } from '@/protocol/generated'
import { KINDS } from '@/test-utils/decisions'
import { clockAnnouncement, clockKind, clockText, secondsLeft } from './clockText'

describe('clockText', () => {
  it.each([
    ['Priority', 10, 'Passing for you in 10 s'],
    ['Priority', 1, 'Passing for you in 1 s'],
    ['DeclareBlockers', 45, 'Declaring no blocks in 45 s'],
    ['ChooseTargets', 10, 'Choosing no targets in 10 s'],
    ['OrderTriggers', 3, 'Ordering the triggers for you in 3 s'],
    ['YesNo', 10, 'Answering no for you in 10 s'],
    ['ChooseOption', 4, 'Choosing the first option for you in 4 s'],
    ['ChooseMode', 10, 'Choosing the first mode for you in 10 s'],
    ['ChooseModeCancel', 2, 'Cancelling for you in 2 s'],
    ['ChooseCards', 10, 'Choosing the first cards for you in 10 s'],
    ['ChooseCardsNone', 6, 'Choosing no cards for you in 6 s'],
    ['ChooseNumber', 5, 'Choosing the smallest number for you in 5 s'],
    ['ChooseNumberCancel', 4, 'Cancelling for you in 4 s'],
    ['OrderCards', 10, 'Keeping the order for you in 10 s'],
  ] as const)('%s with %i s left reads "%s"', (kind, seconds, text) => {
    expect(clockText(kind, seconds)).toBe(text)
  })

  it.each([
    ['Mulligan', 60, 'Keeping this hand for you in 60 s'],
    ['BottomCards', 9, 'Putting the first cards on the bottom for you in 9 s'],
    ['Discard', 5, 'Discarding the first cards for you in 5 s'],
    ['DeclareAttackers', 8, 'Declaring no attacks in 8 s'],
    ['ChooseReplacement', 7, 'Choosing the first effect for you in 7 s'],
    ['DivideAmount', 6, 'Dividing it evenly for you in 6 s'],
  ] as const)('%s, which the server answers at zero, with %i s left reads "%s"', (kind, seconds, text) => {
    expect(clockText(kind, seconds)).toBe(text)
  })

  it('does not claim a consequence it cannot name', () => {
    expect(clockText('PayMana', 5)).toBe('The game goes on without you in 5 s')
  })
})

describe('clockAnnouncement', () => {
  it.each([
    ['Priority', 10, 'You have 10 seconds to answer; after that the game passes for you.'],
    ['Priority', 1, 'You have 1 second to answer; after that the game passes for you.'],
    ['DeclareBlockers', 45, 'You have 45 seconds to answer; after that the game declares no blocks for you.'],
    ['ChooseTargets', 10, 'You have 10 seconds to answer; after that the game chooses no targets for you.'],
    ['OrderTriggers', 10, 'You have 10 seconds to answer; after that the game orders the triggers for you.'],
    ['YesNo', 10, 'You have 10 seconds to answer; after that the game answers no for you.'],
    [
      'ChooseOption',
      10,
      'You have 10 seconds to answer; after that the game chooses the first option for you.',
    ],
    ['ChooseMode', 10, 'You have 10 seconds to answer; after that the game chooses the first mode for you.'],
    ['ChooseModeCancel', 1, 'You have 1 second to answer; after that the game cancels for you.'],
    [
      'ChooseCards',
      10,
      'You have 10 seconds to answer; after that the game chooses the first cards for you.',
    ],
    ['ChooseCardsNone', 10, 'You have 10 seconds to answer; after that the game chooses no cards for you.'],
    ['OrderCards', 10, 'You have 10 seconds to answer; after that the game keeps the order for you.'],
    ['Mulligan', 60, 'You have 60 seconds to answer; after that the game keeps your hand for you.'],
    [
      'Discard',
      5,
      'You have 5 seconds to answer; after that the game discards the first cards of your hand for you.',
    ],
    ['PayMana', 5, 'You have 5 seconds to answer; after that the game goes on without you.'],
  ] as const)('%s with %i s reads "%s"', (kind, seconds, text) => {
    expect(clockAnnouncement(kind, seconds)).toBe(text)
  })
})

describe('clockKind', () => {
  const modes = (can_cancel: boolean): DecisionKind => ({
    ChooseMode: { source: 1, modes: [], min: 1, max: 1, repeat: false, can_cancel },
  })
  const cards = (min: number, max: number): DecisionKind => ({
    ChooseCards: { source: 1, from: [2, 3, 4], min, max },
  })

  it.each([
    ['a mode choice of a spell, which the server takes back', modes(true), 'ChooseModeCancel'],
    ['a mode choice of a trigger, which the server answers with the first mode', modes(false), 'ChooseMode'],
    ['a question', KINDS.YesNo, 'YesNo'],
    ['an option', KINDS.ChooseOption, 'ChooseOption'],
    ['a target choice, whichever way it can be answered', KINDS.ChooseTargets, 'ChooseTargets'],
    [
      'a choice of at least one card, which the server answers with the first cards',
      cards(1, 1),
      'ChooseCards',
    ],
    ['a choice of two cards', cards(2, 3), 'ChooseCards'],
    ['a choice of up to some cards, which the server answers with none', cards(0, 2), 'ChooseCardsNone'],
    ['a number', { ChooseNumber: { source: 1, min: 0, max: 3 } }, 'ChooseNumber'],
    [
      'a number that can be backed out of',
      { ChooseNumber: { source: 1, min: 0, max: 3, can_cancel: true } },
      'ChooseNumberCancel',
    ],
    ['an order of cards', KINDS.OrderCards, 'OrderCards'],
  ])('tells %s as %s', (_what, kind, expected) => {
    expect(clockKind(kind)).toBe(expected)
  })
})

describe('secondsLeft', () => {
  it.each([
    [10_000, 10],
    [9_999, 10],
    [9_000, 9],
    [8_999.5, 9],
    [1_000, 1],
    [1, 1],
    [0, 0],
    [-5, 0],
  ])('%f ms is %i s', (ms, seconds) => {
    expect(secondsLeft(ms)).toBe(seconds)
  })
})
