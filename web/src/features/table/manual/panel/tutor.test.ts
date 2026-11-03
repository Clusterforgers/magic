// Goal: the search grid lists the library by name, not in its order, with an unknown card named plainly, and taking a card previews the shuffle and what the other seats learn and sends the exact `Tutor`; ending without a card sends a shuffle.
import { describe, expect, it } from 'vitest'
import { FIND_NOTHING, foundCards, tutorPreview } from './tutor'

const NAMES: Record<number, string> = { 10: 'Sol Ring', 11: 'Forest', 12: 'Elves', 13: 'Forest' }

describe('the search grid', () => {
  it.each<[string, number[], [number, string][]]>([
    [
      'sorts by name and keeps the order of one name',
      [10, 11, 12, 13],
      [
        [12, 'Elves'],
        [11, 'Forest'],
        [13, 'Forest'],
        [10, 'Sol Ring'],
      ],
    ],
    ['names an unknown card plainly', [99], [[99, 'A card']]],
    ['is empty without a search', [], []],
  ])('%s', (_name, searched, expected) => {
    expect(foundCards(searched, (id) => NAMES[id]).map((card) => [card.id, card.name])).toEqual(expected)
  })
})

describe('taking a card', () => {
  it('asks for a card until one is chosen', () => {
    expect(tutorPreview(undefined, 40)).toEqual({ problem: 'Choose a card to put into your hand.' })
  })

  it('previews the shuffle and sends the card', () => {
    expect(tutorPreview({ id: 10, name: 'Sol Ring' }, 40)).toEqual({
      preview:
        'Sol Ring: library → hand; the 39 other cards are shuffled and the other seats learn only that a card went to your hand',
      apply: 'Put Sol Ring into your hand',
      action: { Tutor: { object: 10 } },
    })
  })

  it('ends without a card by shuffling', () => {
    expect(FIND_NOTHING.action).toEqual({ Shuffle: {} })
  })
})
