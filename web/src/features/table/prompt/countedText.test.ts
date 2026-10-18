/** Goal: for every number of cards asked for (0 to 7) and every number chosen so far, the sentence says how many are chosen out of how many, what is left to do, and when to press Confirm; the refusal names the limit. */
import { describe, expect, it } from 'vitest'
import { cards, countedText, refusalText } from './countedText'

const COUNTS = [0, 1, 2, 3, 4, 5, 6, 7]
const PAIRS = COUNTS.flatMap((need) =>
  COUNTS.filter((chosen) => chosen <= need).map((chosen) => [need, chosen]),
)

describe('cards', () => {
  it.each([
    [0, '0 cards'],
    [1, '1 card'],
    [2, '2 cards'],
    [7, '7 cards'],
  ])('writes %i as %s', (count, words) => {
    expect(cards(count)).toBe(words)
  })
})

describe('countedText', () => {
  it.each(PAIRS)('with %i to choose and %i chosen', (need, chosen) => {
    const text = countedText(need, chosen, 'discard')
    if (need === 0) {
      expect(text).toBe('There are no cards to discard. Press Confirm to go on.')
      return
    }
    expect(text).toContain(`${chosen} of ${need} chosen.`)
    if (chosen === 0) expect(text).toBe(`Choose ${cards(need)} to discard. 0 of ${need} chosen.`)
    else if (chosen < need) {
      expect(text).toBe(
        `${chosen} of ${need} chosen. Choose ${need - chosen} more ${need - chosen === 1 ? 'card' : 'cards'} to discard.`,
      )
    } else expect(text).toBe(`${need} of ${need} chosen. Press Confirm.`)
  })

  it('opens with the intro, and only when asking for the first card', () => {
    expect(countedText(2, 0, 'discard', 'Too many. ')).toMatch(/^Too many\. Choose 2 cards/)
    expect(countedText(2, 1, 'discard', 'Too many. ')).not.toContain('Too many')
  })
})

describe('refusalText', () => {
  it.each(COUNTS.slice(1))('names the limit of %i and the way out', (need) => {
    const text = refusalText(need)
    expect(text).toContain(`Only ${cards(need)} can be chosen.`)
    expect(text).toContain('Choose a chosen card again to put it back.')
  })
})
