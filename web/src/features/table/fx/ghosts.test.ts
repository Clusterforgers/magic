// Goal: a row of cards with ghosts puts each ghost back in front of the card it stood before (or at the end once that card is gone too), keeps ghosts of one place in the order they came, never drops or repeats a card, and leaves a row without ghosts exactly as it was.
import { describe, expect, it } from 'vitest'
import { card } from '@/test-utils/cards'
import type { Ghost } from './model'
import { withGhosts } from './ghosts'

const ghost = (token: number, before: number | null): Ghost => ({
  token,
  object: card({ id: 900 + token }),
  before,
  damage: 0,
})

/** The row as a short string: a card is its id, a ghost is `g` and its token. */
const show = (items: Array<number | Ghost>): string =>
  items.map((item) => (typeof item === 'number' ? String(item) : `g${item.token}`)).join(' ')

/** A small deterministic generator, so a failing case can be replayed. */
function random(seed: number): () => number {
  let state = seed
  return () => {
    state = (state * 1664525 + 1013904223) % 4294967296
    return state / 4294967296
  }
}

describe('a row with ghosts', () => {
  it.each<[string, number[], Ghost[], string]>([
    ['none', [1, 2, 3], [], '1 2 3'],
    ['a ghost before the first card', [2, 3], [ghost(1, 2)], 'g1 2 3'],
    ['a ghost in the middle', [1, 3], [ghost(1, 3)], '1 g1 3'],
    ['a ghost at the end', [1, 2], [ghost(1, null)], '1 2 g1'],
    ['a ghost whose card has gone too', [1, 2], [ghost(1, 7)], '1 2 g1'],
    ['two ghosts before one card, in the order they came', [1, 4], [ghost(1, 4), ghost(2, 4)], '1 g1 g2 4'],
    ['ghosts before different cards', [1, 2, 3], [ghost(1, 3), ghost(2, 1)], 'g2 1 2 g1 3'],
    ['a ghost at the end and one before the last card', [1, 2], [ghost(1, null), ghost(2, 2)], '1 g2 2 g1'],
    ['ghosts and no cards', [], [ghost(1, 5), ghost(2, null)], 'g1 g2'],
  ])('shows %s', (_name, ids, ghosts, expected) => {
    expect(show(withGhosts(ids, ghosts))).toBe(expected)
  })

  it.each(Array.from({ length: 40 }, (_, seed) => seed))(
    'keeps every card once and in order, and every ghost once, whatever the ghosts say (case %i)',
    (seed) => {
      const next = random(seed + 1)
      const pick = (limit: number): number => Math.floor(next() * limit)
      const cards = [...new Set(Array.from({ length: pick(12) }, () => 1 + pick(40)))]
      const ghosts = Array.from({ length: pick(8) }, (_, index) =>
        ghost(index + 1, next() < 0.2 ? null : 1 + pick(45)),
      )
      const items = withGhosts(cards, ghosts)
      expect(items.filter((item): item is number => typeof item === 'number')).toEqual(cards)
      const shown = items.filter((item): item is Ghost => typeof item !== 'number')
      expect(shown.map((item) => item.token).sort((a, b) => a - b)).toEqual(ghosts.map((item) => item.token))
    },
  )
})
