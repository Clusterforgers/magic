// Goal: the dev table shows the whole feature at every seat count and from either side of the table: `?prompt=forecast` has five attackers drawn, ten lines, four outcomes in the key, doomed creatures, damage badges, a lethal seat and the sentences for a screen reader; `?prompt=block` has the viewer facing lethal damage with the forecast "if you do not block" in the prompt and, with a third seat, another defender's double block drawn; and both survive being answered.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderForecastTable } from '@/test-utils/forecastTable'
import { forecastScene } from '../dev/combatScene'
import { buildFixture } from '../dev/fixture'

const COUNTS = [2, 3, 4, 8, 12]

describe.each(COUNTS)('?prompt=forecast at %i seats', (seats) => {
  it('draws five attacks and five blocks, marks the doomed creatures, and shows the lethal seat', () => {
    const t = renderForecastTable({ seats, prompt: 'forecast' })
    const view = must(t.store.getState().view)
    expect(t.lines()).toHaveLength(10)
    expect(t.doomed().length).toBeGreaterThanOrEqual(4)
    expect(t.damage().size).toBeGreaterThanOrEqual(8)
    const defender = must(must(view.combat.attackers[0]).target)
    expect(t.life('Player' in defender ? defender.Player : -1)).toHaveAttribute('data-lethal', 'true')
    const key = must(t.container.querySelector('[data-slot="combat-key"]'))
    expect([...key.querySelectorAll('li')].map((item) => item.textContent)).toEqual([
      'Attack',
      'Attacker dies',
      'Blocker dies',
      'Both die',
      'Both survive',
    ])
    expect(
      within(screen.getByRole('list', { name: 'Combat forecast' })).getAllByRole('listitem'),
    ).toHaveLength(6)
  })
})

describe('?prompt=forecast&turn=you', () => {
  it('has the viewer attack the next seat, with the same forecast on the other side of the table', () => {
    const t = renderForecastTable({
      seats: 3,
      prompt: 'forecast',
      scene: () => forecastScene(buildFixture({ seats: 3, active: 0 })),
    })
    const view = must(t.store.getState().view)
    expect(view.turn.active).toBe(0)
    expect(t.lines()).toHaveLength(10)
    expect(t.life(1)).toHaveAttribute('data-lethal', 'true')
    expect(t.life(0)).toBeNull()
    expect(screen.getByRole('list', { name: 'Combat forecast' })).toHaveTextContent(/^Bo would take 6 damage/)
    expect(t.doomed().length).toBeGreaterThanOrEqual(4)
  })
})

describe.each(COUNTS)('?prompt=block at %i seats', (seats) => {
  it('faces lethal damage, draws the attacks and any block of another defender, and reads out what happens without a block', () => {
    const t = renderForecastTable({ seats, prompt: 'block' })
    const view = must(t.store.getState().view)
    const blocks = view.combat.attackers.flatMap((attacker) => attacker.blockers)
    expect(blocks).toHaveLength(seats === 2 ? 0 : 2)
    expect(t.lines()).toHaveLength(view.combat.attackers.length + blocks.length)
    expect(t.lines().filter((line) => line.pending)).toEqual([])
    expect(t.life(0)).toHaveAttribute('data-lethal', 'true')
    const outcome = must(t.container.querySelector<HTMLElement>('[data-slot="block-outcome"]'))
    expect(outcome).toHaveTextContent('If you do not block')
    expect(within(outcome).getAllByRole('listitem')[0]).toHaveTextContent(
      /^You would take \d+ damage, from \d+ to -?\d+ life, which is lethal\.$/,
    )
  })

  it('goes quietly when answered: lines, marks and the outcome are gone with the decision', async () => {
    const t = renderForecastTable({ seats, prompt: 'block' })
    await t.user.click(screen.getByRole('button', { name: 'No blocks' }))
    expect(t.store.getState().pending).toBeNull()
    expect(t.container.querySelector('[data-slot="block-outcome"]')).toBeNull()
    // The table still carries its forecast: the engine's next batch is what takes it away.
    expect(t.life(0)).toHaveAttribute('data-lethal', 'true')
  })
})

describe('the other dev prompts', () => {
  it.each(['priority', 'attack'])('?prompt=%s shows no forecast: the table carries none', (prompt) => {
    const t = renderForecastTable({ seats: 4, prompt })
    expect(t.doomed()).toEqual([])
    expect(t.damage().size).toBe(0)
    expect(screen.queryByRole('list', { name: 'Combat forecast' })).toBeNull()
  })
})
