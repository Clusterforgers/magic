// Goal: an attack line is drawn strong when the viewer attacks with it or is what it attacks (a planeswalker of theirs too) and faint between two other seats, for a spectator every attack is faint, and the stylesheet says so.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import type { CombatView } from '@/protocol/generated'
import { NO_PENDING } from './pending'
import { planLines } from './plan'

// Cards 10 to 19 belong to seat 1, 20 to 29 to seat 2, 30 to 39 to the viewer (seat 0).
const controllerOf = (id: number): number | undefined =>
  id >= 30 ? 0 : id >= 20 ? 2 : id >= 10 ? 1 : undefined

const combat = (attackers: CombatView['attackers']): CombatView =>
  ({ attackers_declared: true, attackers }) as unknown as CombatView

const attack = (object: number, target: CombatView['attackers'][number]['target']) =>
  ({ object, target, blockers: [] }) as unknown as CombatView['attackers'][number]

const NONE = NO_PENDING

describe('attack lines', () => {
  it.each([
    ['at you', attack(10, { Player: 0 }), true],
    ['by you', attack(30, { Player: 1 }), true],
    ['at your planeswalker', attack(10, { Planeswalker: 31 }), true],
    ['between two other seats', attack(10, { Player: 2 }), false],
    ["at another seat's planeswalker", attack(10, { Planeswalker: 21 }), false],
  ] as const)('marks an attack %s as yours: %s', (_, line, mine) => {
    const [drawn] = planLines({
      combat: combat([line]),
      forecast: null,
      pending: NONE,
      controllerOf,
      viewer: 0,
    })
    expect(drawn?.mine === true).toBe(mine)
  })

  it('marks no attack for a spectator', () => {
    const lines = planLines({
      combat: combat([attack(10, { Player: 0 }), attack(30, { Player: 1 })]),
      forecast: null,
      pending: NONE,
      controllerOf,
      viewer: null,
    })
    expect(lines.every((line) => line.mine !== true)).toBe(true)
  })

  it('draws the attacks of others faint', () => {
    const css = readFileSync(join(import.meta.dirname, 'combat.css'), 'utf8')
    expect(css).toMatch(
      /\.combat-group\[data-kind='attack'\]:not\(\[data-mine\]\):not\(\[data-pending\]\) \{\s*opacity: 0\.35;/,
    )
  })
})
