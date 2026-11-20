// Goal: the Stack panel's top item repeats the engine's forecast as lines, one per card or seat it changes ("Serra Angel: +3/+3 → 7/7"), nothing without a forecast; and a legal target's tip says what the press does and the engine's preview words for it.
import { describe, expect, it } from 'vitest'
import type { ForecastChange } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { pointedTarget, tipWords } from './prompt/targetTip'
import { StackPanel } from './StackPanel'

function withForecast(forecast: ForecastChange[] | undefined) {
  let creature = ''
  renderTable({
    seats: 4,
    ui: <StackPanel />,
    state: (state) => {
      const view = clone(must(state.view))
      const target = must(view.objects.find((o) => o.zone === 'Battlefield' && o.types.includes('Creature')))
      creature = target.name
      const top = must(view.objects.find((o) => o.id === view.stack.cards?.at(-1)))
      if (forecast !== undefined)
        top.forecast = forecast.map((line) =>
          'Object' in line.subject ? { ...line, subject: { Object: target.id } } : line,
        )
      return {
        ...state,
        pending: null,
        view,
        objectsById: Object.fromEntries(view.objects.map((o) => [o.id, o])),
      }
    },
  })
  return {
    creature,
    lines: () =>
      [...document.querySelectorAll('[data-slot="stack-forecast"] li')].map((li) => li.textContent),
  }
}

describe("the top item's forecast line", () => {
  it('names each card and seat it changes with the engine words', () => {
    const { creature, lines } = withForecast([
      { subject: { Object: 0 }, change: { Stats: { power: [4, 7], toughness: [4, 7] } } },
      { subject: { Player: 1 }, change: { Life: { life: 40, life_after: 37 } } },
    ])
    expect(lines()).toEqual([`${creature}: +3/+3 → 7/7`, 'Bo: −3 → 37'])
  })

  it('says nothing without a forecast', () => {
    const { lines } = withForecast(undefined)
    expect(lines()).toEqual([])
  })
})

describe("a target's tip", () => {
  it.each([
    [null, 'Click to target'],
    ['3 damage · dies', 'Click to target · 3 damage · dies'],
  ])('reads %j as %s', (forecast, words) => {
    expect(tipWords(forecast)).toBe(words)
  })

  it('points only at a legal card of the table', () => {
    document.body.innerHTML =
      '<div data-area="own"><button data-object-id="7" data-highlight="legal"><span id="in"></span></button><button data-object-id="8" data-highlight="dimmed"></button></div>'
    expect(pointedTarget(document.getElementById('in'))).toBe(7)
    expect(pointedTarget(document.querySelector('[data-object-id="8"]'))).toBeNull()
    expect(pointedTarget(null)).toBeNull()
  })
})
