/** Goal: the life each seat would have after combat damage is one small chip beside its life figure (the change in red or green, "0" when all is blocked, a skull when lethal) that takes no row of its own, is named by a sentence, has the forecast's sentences as its tooltip, and is gone while there is no forecast. */
import { describe, expect, it } from 'vitest'
import type { PlayerForecast } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderForecastTable } from '@/test-utils/forecastTable'
import { lifeDelta, lifeTrend } from './LifeForecast'

const forecastOf = (player: Partial<PlayerForecast>): PlayerForecast => ({
  player: 0,
  damage: 0,
  life: 20,
  life_after: 20,
  lethal: false,
  ...player,
})

describe('the figures of the chip', () => {
  it.each([
    { name: 'damage', player: { damage: 3, life: 40, life_after: 37 }, delta: '-3', trend: 'down' },
    {
      name: 'lethal damage',
      player: { damage: 9, life: 5, life_after: -4, lethal: true },
      delta: '-9',
      trend: 'down',
    },
    {
      name: 'lifelink of a blocker that outweighs the damage',
      player: { damage: 2, life: 10, life_after: 13 },
      delta: '+3',
      trend: 'up',
    },
    { name: 'everything blocked', player: {}, delta: '0', trend: 'same' },
    { name: 'a doubled life gain', player: { life: 20, life_after: 26 }, delta: '+6', trend: 'up' },
  ])('reads $name as $delta', ({ player, delta, trend }) => {
    const forecast = forecastOf(player)
    expect(lifeDelta(forecast)).toBe(delta)
    expect(lifeTrend(forecast)).toBe(trend)
  })
})

describe('the chip on a seat panel', () => {
  it.each([
    {
      name: 'damage',
      player: { damage: 4, life: 20, life_after: 16 },
      text: '-4',
      trend: 'down',
      lethal: false,
    },
    {
      name: 'lifelink',
      player: { damage: 1, life: 20, life_after: 24 },
      text: '+4',
      trend: 'up',
      lethal: false,
    },
    { name: 'a block', player: {}, text: '0', trend: 'same', lethal: false },
    {
      name: 'lethal damage',
      player: { damage: 25, life: 20, life_after: -5, lethal: true },
      text: '-25',
      trend: 'down',
      lethal: true,
    },
  ])('shows $name as $text', ({ player, text, trend, lethal }) => {
    const table = renderForecastTable({ seats: 2 })
    table.setView({
      combat_forecast: { ...table.forecast(), players: [forecastOf(player)] },
    })
    const chip = must(table.life(0))
    expect(chip).toHaveTextContent(text)
    expect(chip).toHaveAttribute('data-trend', trend)
    expect(chip.hasAttribute('data-lethal')).toBe(lethal)
    expect(chip).toHaveAttribute('role', 'img')
    expect(chip.title.split('\n').length).toBeGreaterThan(1)
  })

  it('sits inside the life area of the pod and gives it no row', () => {
    const table = renderForecastTable({ seats: 2 })
    const chip = must(table.life(0))
    const pod = must(chip.closest('section[data-seat]'))
    expect(must(chip.parentElement).contains(pod.querySelector('p'))).toBe(true)
    expect(pod.querySelector('[data-slot="pod-status"]')?.contains(chip) ?? false).toBe(false)
  })

  it('is gone while no forecast holds', () => {
    const table = renderForecastTable({ seats: 2 })
    table.setView({ combat_forecast: null })
    expect(table.life(0)).toBeNull()
  })
})
