/** A table of seat 0 over a combat that carries a forecast, with a fixed layout and readers of what it draws. */
import { RectProviderContext } from '@/features/table/combat/rects'
import { forecastScene } from '@/features/table/dev/combatScene'
import { buildFixture } from '@/features/table/dev/fixture'
import type { Fixture } from '@/features/table/dev/fixture'
import { DEV_PROMPTS, DEV_TABLES } from '@/features/table/dev/prompts'
import { GameTable } from '@/features/table/GameTable'
import type { TableOptions } from '@/features/table/tableOptions'
import type { CombatForecast, ObjectId, PlayerId } from '@/protocol/generated'
import type { PlayState } from '@/play/store'
import { must } from './batches'
import { fakeRects } from './rects'
import type { FakeRects } from './rects'
import { renderTable } from './tableHarness'
import type { TableHarness } from './tableHarness'

/** One line of the overlay as the page shows it. */
export interface DrawnLineView {
  key: string
  /** Where it starts and ends, as anchor keys such as `card:12` and `seat:0`. */
  from: string
  to: string
  kind: string
  pending: boolean
  outcome: string | null
  /** The path of the curve. */
  d: string
}

/** What a test of the forecast on the table reads. */
export interface ForecastTable extends TableHarness, FakeRects {
  /** The overlay, or null while nothing is drawn. */
  overlay(): HTMLElement | null
  /** The lines of the strong layer, in drawing order. */
  lines(): DrawnLineView[]
  /** The cards that look doomed. */
  doomed(): ObjectId[]
  /** The text of the damage badge of each card that has one. */
  damage(): Map<ObjectId, string>
  /** The forecast line on a seat's panel, or null. */
  life(seat: PlayerId): HTMLElement | null
  /** The card button of an object. */
  card(id: ObjectId): HTMLElement
  /** The forecast now in the table. */
  forecast(): CombatForecast
  /** Changes the table view, the way a batch would. */
  setView(change: Partial<NonNullable<PlayState['view']>>): void
}

/** Options of `renderForecastTable`. */
export interface ForecastTableOptions {
  seats?: number
  /**
   * The dev prompt to face (`block`, `attack`), on the table its moment needs (`dev/prompts.ts`); the table then has
   * no decision when left out, and shows the `forecastScene` unless `scene` says otherwise.
   */
  prompt?: string
  /** How to turn the plain fixture into the moment to look at. */
  scene?: (fixture: Fixture) => Fixture
  /** Where the layout starts: `{ mode: 'grid' }` draws every seat's board, the worst case at 12 seats. */
  layout?: TableOptions
}

/** Renders the table over the moment asked for, with a rect provider in place of the DOM. */
export function renderForecastTable({
  seats = 2,
  prompt,
  scene = prompt === undefined ? forecastScene : (DEV_TABLES[prompt] ?? ((fixture) => fixture)),
  layout,
}: ForecastTableOptions = {}): ForecastTable {
  const rects = fakeRects()
  const fixture = scene(buildFixture({ seats }))
  const decision = prompt === undefined ? null : (DEV_PROMPTS[prompt]?.(fixture) ?? null)
  const table = renderTable({
    fixture,
    options: layout,
    state: (state) => ({ ...state, pending: decision }),
    ui: (
      <RectProviderContext value={rects.provider}>
        <GameTable />
      </RectProviderContext>
    ),
  })
  const root = table.container
  const card = (id: ObjectId): HTMLElement =>
    must(root.querySelector<HTMLElement>(`button[data-object-id="${id}"]`), `card ${id}`)
  return {
    ...table,
    ...rects,
    overlay: () => root.querySelector<HTMLElement>('[data-slot="combat-overlay"]'),
    lines: () =>
      [...root.querySelectorAll<SVGGElement>('[data-layer="lines"] g[data-line]')].map((group) => ({
        key: must(group.getAttribute('data-line')),
        from: must(group.getAttribute('data-from')),
        to: must(group.getAttribute('data-to')),
        kind: must(group.getAttribute('data-kind')),
        pending: group.hasAttribute('data-pending'),
        outcome: group.getAttribute('data-outcome'),
        d: must(group.querySelector('.combat-line')?.getAttribute('d')),
      })),
    doomed: () =>
      [...root.querySelectorAll<HTMLElement>('button[data-doomed]')].map((element) =>
        Number(element.getAttribute('data-object-id')),
      ),
    damage: () =>
      new Map(
        [...root.querySelectorAll<HTMLElement>('button[data-object-id] [data-slot="forecast-damage"]')].map(
          (badge) => [
            Number(badge.closest('button')?.getAttribute('data-object-id')),
            badge.textContent ?? '',
          ],
        ),
      ),
    life: (seat) => root.querySelector<HTMLElement>(`[data-seat="${seat}"] [data-slot="life-forecast"]`),
    card,
    forecast: () => must(must(table.store.getState().view).combat_forecast),
    setView: (change) => table.update((state) => ({ view: { ...must(state.view), ...change } })),
  }
}
