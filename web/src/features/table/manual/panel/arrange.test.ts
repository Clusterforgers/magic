// Goal: the arrangement of the cards looked at starts with every card on top in order, sends a card to the end of another list, moves a card within its list and never past either end, reads as a surveil, a scry or a reorder the way the engine does, builds an `ArrangeTop` of exactly the cards looked at, words its button and its preview, and treats a look as open until the seat's next put-back or shuffle.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { ManualAction } from '@/protocol/generated'
import { pub } from '@/test-utils/turnEvents'
import {
  applyWords,
  arrangeAction,
  destinationOf,
  isUnchanged,
  kindOf,
  lookIsPending,
  moveWithin,
  previewWords,
  sendTo,
  startArrangement,
} from './arrange'
import type { Arrangement } from './arrange'

const LOOKED = [10, 11, 12]
const NAMES: Record<number, string> = { 10: 'Elves', 11: 'Island', 12: 'Sol Ring' }
const nameOf = (id: number): string => NAMES[id] ?? 'a card'

const arranged = (top: number[], bottom: number[] = [], graveyard: number[] = []): Arrangement => ({
  top,
  bottom,
  graveyard,
})

describe('the start', () => {
  it('has every card on top in the order looked at', () => {
    expect(startArrangement(LOOKED)).toEqual(arranged([10, 11, 12]))
    expect(isUnchanged(startArrangement(LOOKED), LOOKED)).toBe(true)
  })
})

describe('sending a card', () => {
  it.each<[string, Arrangement, number, 'top' | 'bottom' | 'graveyard', Arrangement]>([
    ['to the bottom', arranged([10, 11, 12]), 11, 'bottom', arranged([10, 12], [11])],
    ['to the graveyard', arranged([10, 11, 12]), 10, 'graveyard', arranged([11, 12], [], [10])],
    ['to the end of the bottom list', arranged([12], [10], [11]), 12, 'bottom', arranged([], [10, 12], [11])],
    ['back to the top, at the end', arranged([10], [11], [12]), 11, 'top', arranged([10, 11], [], [12])],
    ['where it already is', arranged([10, 11], [12]), 12, 'bottom', arranged([10, 11], [12])],
    ['a card that was not looked at', arranged([10, 11, 12]), 99, 'bottom', arranged([10, 11, 12])],
  ])('%s', (_name, from, card, to, expected) => {
    expect(sendTo(from, card, to)).toEqual(expected)
  })

  it('tells where a card goes', () => {
    const now = arranged([10], [11], [12])
    expect([10, 11, 12, 99].map((card) => destinationOf(now, card))).toEqual([
      'top',
      'bottom',
      'graveyard',
      undefined,
    ])
  })
})

describe('moving a card in its list', () => {
  it.each<[string, Arrangement, number, -1 | 1, Arrangement]>([
    ['up', arranged([10, 11, 12]), 11, -1, arranged([11, 10, 12])],
    ['down', arranged([10, 11, 12]), 11, 1, arranged([10, 12, 11])],
    ['up from the first place', arranged([10, 11, 12]), 10, -1, arranged([10, 11, 12])],
    ['down from the last place', arranged([10, 11, 12]), 12, 1, arranged([10, 11, 12])],
    ['in the bottom list', arranged([10], [11, 12]), 12, -1, arranged([10], [12, 11])],
    ['in the graveyard list', arranged([], [], [10, 11]), 10, 1, arranged([], [], [11, 10])],
    ['a card not held', arranged([10, 11]), 99, 1, arranged([10, 11])],
  ])('%s', (_name, from, card, step, expected) => {
    expect(moveWithin(from, card, step)).toEqual(expected)
  })

  it('is no change when the same cards are on top in another order', () => {
    expect(isUnchanged(moveWithin(startArrangement(LOOKED), 12, -1), LOOKED)).toBe(false)
  })
})

describe('the kind and the action', () => {
  it.each<[string, Arrangement, 'surveil' | 'scry' | 'reorder']>([
    ['nothing moves', arranged([10, 11, 12]), 'reorder'],
    ['a new order', arranged([11, 10, 12]), 'reorder'],
    ['one to the graveyard', arranged([10, 11], [], [12]), 'surveil'],
    ['one to the bottom', arranged([10, 11], [12]), 'scry'],
    ['one each way', arranged([10], [11], [12]), 'scry'],
  ])('%s is a %s', (_name, arrangement, kind) => {
    expect(kindOf(arrangement)).toBe(kind)
  })

  it('sends exactly the looked-at cards, each once, in the lists as arranged', () => {
    const now = sendTo(sendTo(moveWithin(startArrangement(LOOKED), 12, -1), 10, 'bottom'), 11, 'graveyard')
    const action: ManualAction = arrangeAction(now)
    expect(action).toEqual({ ArrangeTop: { top: [12], bottom: [10], graveyard: [11] } })
    const sent =
      'ArrangeTop' in action
        ? [...action.ArrangeTop.top, ...action.ArrangeTop.bottom, ...action.ArrangeTop.graveyard]
        : []
    expect([...sent].sort()).toEqual([...LOOKED].sort())
  })
})

describe('the words', () => {
  it.each<[string, Arrangement, string]>([
    ['no change', arranged([10, 11, 12]), 'Keep all 3 on top'],
    ['a new order', arranged([11, 10, 12]), 'Reorder the 3 on top'],
    [
      'two to the bottom, one to the graveyard',
      arranged([], [10, 11], [12]),
      'Put 2 on the bottom, 1 in the graveyard',
    ],
    ['one to the graveyard', arranged([10, 11], [], [12]), 'Put 1 in the graveyard'],
    ['one to the bottom', arranged([10, 11], [12]), 'Put 1 on the bottom'],
  ])('the button for %s reads "%s"', (_name, arrangement, words) => {
    expect(applyWords(arrangement, LOOKED)).toBe(words)
  })

  it('words a single card', () => {
    expect(applyWords(arranged([10]), [10])).toBe('Keep it on top')
  })

  it.each<[string, Arrangement, string]>([
    ['no change', arranged([10, 11, 12]), 'Nothing moves: the cards stay on top in this order.'],
    ['a scry', arranged([10, 11], [12]), 'Scry: Elves, Island stay on top; Sol Ring goes to the bottom.'],
    [
      'a surveil',
      arranged([12], [], [10, 11]),
      'Surveil: Sol Ring stays on top; Elves, Island go to the graveyard.',
    ],
    ['a reorder', arranged([12, 10, 11]), 'Reorder: Sol Ring, Elves, Island stay on top.'],
  ])('the preview for %s', (_name, arrangement, words) => {
    expect(previewWords(arrangement, LOOKED, nameOf)).toBe(words)
  })
})

describe('a look that waits', () => {
  const entry = (action: ManualAction, player = 0): LogEntry => ({
    seq: 0,
    event: pub({ ManualAction: { player, action } }),
  })
  const arrange: ManualAction = { ArrangeTop: { top: [10], bottom: [], graveyard: [] } }
  it.each<[string, LogEntry[], boolean]>([
    ['an empty log', [], false],
    ['a look', [entry({ LookTop: { count: 3 } })], true],
    ['a look then a draw', [entry({ LookTop: { count: 3 } }), entry({ Draw: { count: 1 } })], true],
    ['a look then a put-back', [entry({ LookTop: { count: 3 } }), entry(arrange)], false],
    ['a look then a shuffle', [entry({ LookTop: { count: 3 } }), entry({ Shuffle: {} })], false],
    ['a put-back then a new look', [entry(arrange), entry({ LookTop: { count: 2 } })], true],
    ['another seat looked', [entry({ LookTop: { count: 3 } }, 1)], false],
  ])('%s: %s', (_name, log, pending) => {
    expect(lookIsPending(log, 0)).toBe(pending)
  })
})
