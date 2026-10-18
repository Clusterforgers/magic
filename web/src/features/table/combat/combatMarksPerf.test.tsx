// Goal: the forecast costs a crowded table nothing it does not use, at the worst case of 12 seats with 40 permanents each (480 cards on screen): a forecast arriving, changing or going redraws only the creatures it mentions and seats it attacks, a change to one creature redraws that card alone, choosing a block redraws only the cards a block touches and the creatures the forecast had marked, and the lines are measured once per change, not once per card.
import { screen } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { CombatForecast, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderForecastTable } from '@/test-utils/forecastTable'

// Every render of a card face is one render of a card.
const faces = vi.hoisted(() => new Map<number, number>())

vi.mock('../CardFace', async (importOriginal) => {
  const original = await importOriginal<typeof import('../CardFace')>()
  const { createElement } = await import('react')
  return {
    ...original,
    CardFace(props: ComponentProps<typeof original.CardFace>) {
      faces.set(props.object.id, (faces.get(props.object.id) ?? 0) + 1)
      return createElement(original.CardFace, props)
    },
  }
})

const SEATS = 12
const redrawn = (): ObjectId[] => [...faces.keys()].sort((a, b) => a - b)

beforeEach(() => faces.clear())

describe('a table of 12 seats with a forecast', () => {
  it('draws every card once to begin with, the forecast included', () => {
    const t = renderForecastTable({ seats: SEATS, layout: { mode: 'grid' } })
    expect(faces.size).toBeGreaterThan(400)
    for (const count of faces.values()) expect(count).toBeLessThanOrEqual(2)
    expect(t.doomed().length).toBeGreaterThan(0)
  })

  it('redraws only the creature whose damage changed when the forecast changes in one place', () => {
    const t = renderForecastTable({ seats: SEATS, layout: { mode: 'grid' } })
    const forecast = t.forecast()
    const target = must(forecast.creatures.find((creature) => creature.damage > 0 && !creature.dies))
    faces.clear()
    const changed: CombatForecast = {
      ...forecast,
      creatures: forecast.creatures.map((creature) =>
        creature.object === target.object ? { ...creature, damage: creature.damage + 1 } : creature,
      ),
    }
    t.setView({ combat_forecast: changed })
    expect(redrawn()).toEqual([target.object])
    expect(t.damage().get(target.object)).toBe(`-${target.damage + 1}`)
  })

  it('redraws only the creatures it mentioned when the forecast goes, and again when it comes back', () => {
    const t = renderForecastTable({ seats: SEATS, layout: { mode: 'grid' } })
    const forecast = t.forecast()
    const marked = forecast.creatures
      .filter((creature) => creature.damage > 0 || creature.dies)
      .map((creature) => creature.object)
      .sort((a, b) => a - b)
    expect(marked.length).toBeGreaterThan(5)
    faces.clear()
    t.setView({ combat_forecast: null })
    expect(redrawn()).toEqual(marked)
    faces.clear()
    t.setView({ combat_forecast: forecast })
    expect(redrawn()).toEqual(marked)
  })

  it('redraws no card at all when the forecast changes only in what no card shows', () => {
    const t = renderForecastTable({ seats: SEATS, layout: { mode: 'grid' } })
    const forecast = t.forecast()
    faces.clear()
    t.setView({ combat_forecast: { ...forecast, attackers: [...forecast.attackers].reverse() } })
    expect(redrawn()).toEqual([])
  })

  it('redraws no card when the life of a seat changes under the forecast: that is a panel, not a card', () => {
    const t = renderForecastTable({ seats: SEATS, layout: { mode: 'grid' } })
    const forecast = t.forecast()
    faces.clear()
    t.setView({
      combat_forecast: {
        ...forecast,
        players: forecast.players.map((player) => ({
          ...player,
          life: player.life + 1,
          life_after: player.life_after + 1,
        })),
      },
    })
    expect(redrawn()).toEqual([])
    expect(t.life(0)).toHaveTextContent('6')
  })

  it('measures the lines once for the table, not once per card, whatever the forecast does', () => {
    const t = renderForecastTable({ seats: SEATS, layout: { mode: 'grid' } })
    const calls = t.measure.mock.calls.length
    expect(calls).toBeLessThanOrEqual(2)
    const forecast = t.forecast()
    t.setView({ combat_forecast: null })
    t.setView({ combat_forecast: forecast })
    expect(t.measure.mock.calls.length).toBe(calls)
  })
})

describe('a block chosen at 12 seats', () => {
  /** The table facing the declare-blockers decision, with the cards a block may touch and those the forecast marks. */
  function facing() {
    const t = renderForecastTable({ seats: SEATS, prompt: 'block', layout: { mode: 'grid' } })
    const view = must(t.store.getState().view)
    const kind = must(t.store.getState().pending).decision.kind
    const options = must('DeclareBlockers' in kind ? kind.DeclareBlockers : null)
    const blocker = must(options.blockers[0]?.object)
    const attacker = must(options.blockers[0]?.can_block[0])
    const marked = t
      .forecast()
      .creatures.filter((creature) => creature.damage > 0 || creature.dies)
      .map((creature) => creature.object)
    // Only the viewer's cards and the attacking seat's can be marked or noted by a block; nobody else's.
    const mayChange = new Set<ObjectId>(
      view.objects
        .filter((object) => object.controller === 0 || object.controller === view.turn.active)
        .map((object) => object.id),
    )
    for (const id of marked) mayChange.add(id)
    return { t, blocker, attacker, marked, mayChange }
  }

  it('redraws the creatures the forecast marked and the cards a block touches, and no card of anyone else', async () => {
    const { t, blocker, attacker, marked, mayChange } = facing()
    expect(marked.length).toBeGreaterThan(0)
    faces.clear()
    await t.user.click(t.card(blocker))
    await t.user.click(t.card(attacker))
    const drawn = redrawn()
    for (const id of marked) expect(drawn).toContain(id)
    expect(drawn.filter((id) => !mayChange.has(id))).toEqual([])
    expect(drawn.length).toBeLessThan(120)
    expect(t.damage().size).toBe(0)
  })

  it('redraws the same creatures when the block is taken back and the forecast returns', async () => {
    const { t, blocker, attacker, marked, mayChange } = facing()
    await t.user.click(t.card(blocker))
    await t.user.click(t.card(attacker))
    expect(t.damage().size).toBe(0)
    faces.clear()
    await t.user.click(screen.getByRole('button', { name: new RegExp(`^Remove the block of `) }))
    const drawn = redrawn()
    for (const id of marked) expect(drawn).toContain(id)
    expect(drawn.filter((id) => !mayChange.has(id))).toEqual([])
    expect(t.damage().size).toBeGreaterThan(0)
  })
})
