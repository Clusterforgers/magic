/** Goal: the top stack item's forecast draws one dark badge on each card it changes and a chip on each seat pod whose life it changes, at two to four seats, and the badges leave with the item. */
import { describe, expect, it } from 'vitest'
import { buildFixture } from '@/features/table/dev/fixture'
import type { Fixture } from '@/features/table/dev/fixture'
import { forecastStackTable } from '@/features/table/dev/previewScenes'
import type { ForecastChange, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

/** The scene's Giant Growth, which also costs the next seat 3 life. */
function scene(seats: number): { fixture: Fixture; target: ObjectId; seat: number } {
  const fixture = forecastStackTable(buildFixture({ seats }))
  const { view } = fixture
  const top = must(view.stack.cards?.at(-1))
  const spell = must(view.objects.find((object) => object.id === top))
  const line = must(spell.forecast?.[0])
  if (!('Object' in line.subject)) throw new Error('the scene pumps a creature')
  const seat = (view.viewer + 1) % view.players.length
  const life: ForecastChange = { subject: { Player: seat }, change: { Life: { life: 20, life_after: 17 } } }
  const objects = view.objects.map((object) =>
    object.id === top ? { ...object, forecast: [...(object.forecast ?? []), life] } : object,
  )
  return { fixture: { ...fixture, view: { ...view, objects } }, target: line.subject.Object, seat }
}

const badges = (container: HTMLElement, place: 'card' | 'pod'): HTMLElement[] => [
  ...container.querySelectorAll<HTMLElement>(`[data-slot='stack-forecast'][data-place='${place}']`),
]

describe.each([2, 3, 4])('at %i seats', (seats) => {
  it('badges the target card and the seat that loses life, and nothing else', () => {
    const { fixture, target } = scene(seats)
    const table = renderTable({ fixture })
    const cards = badges(table.container, 'card')
    expect(cards.map((badge) => badge.textContent)).toEqual(['+3/+3 → 7/7'])
    expect(must(cards[0]).closest('[data-object-id]')?.getAttribute('data-object-id')).toBe(String(target))
    expect(must(cards[0]).getAttribute('aria-label')).toBe('Forecast: +3/+3 → 7/7')
    expect(badges(table.container, 'pod').map((badge) => badge.textContent)).toEqual(['−3 → 17'])
    table.unmount()
  })

  it('takes the badges away once the item has left the stack', () => {
    const { fixture } = scene(seats)
    const table = renderTable({ fixture })
    table.update((state) => {
      const view = must(state.view)
      return { view: { ...view, stack: { count: 0, cards: [] } } }
    })
    expect(badges(table.container, 'card')).toEqual([])
    expect(badges(table.container, 'pod')).toEqual([])
    table.unmount()
  })
})
