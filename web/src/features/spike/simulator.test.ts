// Goal: the simulator is deterministic per seed and sized as configured, so performance runs are comparable.
import { describe, expect, it } from 'vitest'
import { createBoard, createRng, nextPatches } from './simulator'

const config = { players: 4, permanentsPerPlayer: 37, handSize: 7, seed: 1 }

describe('simulator', () => {
  it('builds players x (battlefield + hand) objects with unique ids', () => {
    const board = createBoard(config)
    expect(board).toHaveLength(4 * (37 + 7))
    expect(new Set(board.map((object) => object.id)).size).toBe(board.length)
  })

  it.each([1, 2, 99])('seed %i yields identical boards and patch streams', (seed) => {
    const run = () => {
      const board = createBoard({ ...config, seed })
      const rng = createRng(seed)
      return [board, nextPatches(board, rng, 20)]
    }
    expect(run()).toEqual(run())
  })

  it('moves a hand card onto the battlefield every 20th tick', () => {
    const board = createBoard(config)
    const withMove = nextPatches(board, createRng(3), 20)
    expect(withMove.some((patch) => patch.changes.zone === 'battlefield')).toBe(true)
    const without = nextPatches(board, createRng(3), 21)
    expect(without.some((patch) => patch.changes.zone === 'battlefield')).toBe(false)
  })
})
