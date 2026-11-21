// Goal: on a crowded board (60 and 120 mana sources of every kind: basics, duals, rocks, Signets, Treasures, filter lands, Cabal Coffers, creature-only mana) the payment prompt's work stays quick: the sources that fit, the game's payment laid out and the rest paid for me, by arithmetic and rendered on the dev table, and permanents alike give the same answer.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { ManaOption } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { cost, poolOf, source } from '@/test-utils/mana'
import { TEST_VIEWPORT } from '@/test-utils/tableHarness'
import { fixtureGame } from '../dev/devGame'
import { GameTable } from '../GameTable'
import { TableOptionsContext } from '../tableOptions'
import { completable, completion, fittingOptions, type Reach, type Spend } from './payment'

const COLOURS = 'WUBRG'

/** `count` permanents of mana: every kind a crowded Commander board has, in turn. */
function board(count: number): ManaOption[] {
  const all: ManaOption[] = []
  for (let n = 0; n < count; n += 1) {
    const id = 100 + n
    const letter = COLOURS[n % 5] ?? 'G'
    const next = COLOURS[(n + 1) % 5] ?? 'W'
    switch (n % 10) {
      case 1:
        all.push(source(id, letter), source(id, next, 1))
        break
      case 3:
        all.push({ ...source(id, 'C'), count: 2 })
        break
      case 5:
        all.push({ ...source(id, letter), also: [source(id, next).produces], cost: cost('{1}') })
        break
      case 6:
        for (const [ability, pair] of ['WW', 'WU', 'UU'].entries()) {
          const [first, second] = [...pair].map((one) => source(id, one).produces)
          all.push({
            ...source(id, must(pair[0]), ability),
            produces: must(first),
            also: [must(second)],
            cost: cost('{W/U}'),
          })
        }
        break
      case 7:
        for (const [ability, one] of [...COLOURS].entries())
          all.push({ ...source(id, one, ability), sacrifice: true })
        break
      case 8:
        all.push({ ...source(id, 'B'), count: 4, cost: cost('{2}') })
        break
      case 9:
        for (const [ability, one] of [...COLOURS].entries()) {
          all.push({ ...source(id, one, ability), only: [{ Spell: { types: ['Creature'] } }] })
        }
        break
      default:
        all.push(source(id, letter))
    }
  }
  return all
}

const CREATURE = { kind: 'Spell', traits: { types: 1 << 2, colors: 0 } } as const

/** The time the prompt's arithmetic takes for one render: every permanent's fitting options, then the payment laid out. */
function timed(count: number, price: string): { ms: number; legal: number; paid: boolean } {
  const sources = board(count)
  const reach: Reach = { sources, maxLife: 0, spend: CREATURE as unknown as Reach['spend'] }
  const owed = cost(price)
  const pool = poolOf('')
  const ids = [...new Set(sources.map((one) => one.source.object))]
  const by = new Map(ids.map((id) => [id, sources.filter((one) => one.source.object === id)]))
  const start = performance.now()
  const legal = ids.filter((id) => fittingOptions(owed, pool, [], by.get(id) ?? [], [], reach).length > 0)
  const whole = completion(owed, pool, [], [], reach)
  const spends: Spend[] = [...(whole?.spends ?? [])]
  for (const id of ids) fittingOptions(owed, pool, spends, by.get(id) ?? [], [], reach)
  for (const id of ids) fittingOptions(owed, pool, spends.slice(0, 2), by.get(id) ?? [], [], reach)
  return { ms: performance.now() - start, legal: legal.length, paid: whole !== undefined }
}

describe('a crowded board of mana', () => {
  it.each([
    [60, '{3}{G}{G}', 4_000],
    [60, '{W}{U}{B}{R}{G}', 4_000],
    [60, '{2}{G/U}{G/U}', 4_000],
    [120, '{3}{G}{G}', 6_000],
    [120, '{W}{U}{B}{R}{G}', 6_000],
    [120, '{6}{U}{U}', 6_000],
  ])('with %i sources works out %s within %i ms', (count, price, bound) => {
    const { ms, legal, paid } = timed(count, price)
    expect(paid).toBe(true)
    expect(legal).toBeGreaterThan(count / 2)
    expect(ms).toBeLessThan(bound)
  })

  it('answers alike for permanents alike', () => {
    const sources = [
      source(1, 'G'),
      source(2, 'G'),
      source(3, 'W'),
      { ...source(4, 'W'), cost: cost('{W/U}') },
    ]
    const reach: Reach = { sources, maxLife: 0 }
    const owed = cost('{G}{G}')
    const first: Spend = { kind: 'tap', option: must(sources[0]) }
    const second: Spend = { kind: 'tap', option: must(sources[1]) }
    const plains: Spend = { kind: 'tap', option: must(sources[2]) }
    expect(completable(owed, poolOf(''), [first], [], reach)).toBe(true)
    expect(completable(owed, poolOf(''), [second], [], reach)).toBe(true)
    expect(completable(owed, poolOf(''), [first, second], [], reach)).toBe(true)
    expect(completable(owed, poolOf(''), [plains], [], reach)).toBe(false)
    expect(completable(owed, poolOf(''), [first, plains], [], reach)).toBe(false)
  })
})

/** The dev table facing the crowded payment `prompt`, rendered. */
function crowdTable(prompt: string): { ms: number } {
  const game = fixtureGame({ seats: 4, prompt })
  const start = performance.now()
  render(
    <GameContext value={game}>
      <TableOptionsContext value={{ viewport: TEST_VIEWPORT }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return { ms: performance.now() - start }
}

describe('the crowded payment on the table', () => {
  it.each([
    ['pay-crowd', 10_000],
    ['pay-crowd-120', 15_000],
  ])(
    '%s renders, lays out the game payment and confirms it within %i ms',
    async (prompt, bound) => {
      const user = userEvent.setup()
      const start = performance.now()
      const { ms } = crowdTable(prompt)
      expect(ms).toBeLessThan(bound)
      await user.click(screen.getByRole('button', { name: 'Pay for me' }))
      expect(screen.getByRole('button', { name: 'Confirm payment' })).toBeEnabled()
      expect(performance.now() - start).toBeLessThan(bound)
    },
    30_000,
  )
})
