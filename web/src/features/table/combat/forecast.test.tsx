// Goal: what the table draws of a forecast is decided in one place: the table's own forecast while no block is pending and nothing while blocks are being chosen, per creature (only when there is damage to show or a death) and per player, through hooks that redraw only what changed, and that leave the many cards and panels the forecast does not mention alone when the first block is chosen.
import { act, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { CombatForecast, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import {
  creatureOutlook,
  isOutdated,
  playerOutlook,
  previewFor,
  useBlockPreview,
  useCreatureOutlook,
  usePlayerOutlook,
} from './forecast'
import { PendingCombatContext, createPendingCombatStore } from './pending'

const FORECAST: CombatForecast = {
  attackers: [],
  creatures: [
    { object: 1, damage: 4, dies: true },
    { object: 2, damage: 2, dies: false },
    { object: 3, damage: 0, dies: false },
    { object: 4, damage: 0, dies: true },
  ],
  players: [
    { player: 0, damage: 7, life: 5, life_after: -2, lethal: true },
    { player: 2, damage: 0, life: 40, life_after: 40, lethal: false },
  ],
}

describe('previewFor, the one place that decides which forecast is drawn', () => {
  const none: ReadonlyMap<ObjectId, ObjectId> = new Map()
  const some: ReadonlyMap<ObjectId, ObjectId> = new Map([[5, 1]])

  it.each<[string, CombatForecast | null, ReadonlyMap<ObjectId, ObjectId>, CombatForecast | null]>([
    ['no forecast and no block', null, none, null],
    ['no forecast and a block', null, some, null],
    ['a forecast and no block: the table is right about the answer', FORECAST, none, FORECAST],
    ['a forecast and a block: the table knows nothing of the block', FORECAST, some, null],
  ])('with %s', (_name, table, blocks, expected) => {
    expect(previewFor(table, blocks)).toBe(expected)
  })

  it('counts a forecast as out of date as soon as one block is pending', () => {
    expect(isOutdated(none)).toBe(false)
    expect(isOutdated(some)).toBe(true)
  })
})

describe('creatureOutlook', () => {
  it.each<[string, CombatForecast | null, ObjectId, { damage: number; dies: boolean } | null]>([
    ['damage that kills', FORECAST, 1, { damage: 4, dies: true }],
    ['damage that is survived', FORECAST, 2, { damage: 2, dies: false }],
    ['a creature nothing happens to', FORECAST, 3, null],
    ['a death without damage (no toughness)', FORECAST, 4, { damage: 0, dies: true }],
    ['a creature the forecast does not list', FORECAST, 99, null],
    ['no forecast', null, 1, null],
  ])('says %s', (_name, forecast, id, expected) => {
    expect(creatureOutlook(forecast, id)).toEqual(expected)
  })
})

describe('playerOutlook', () => {
  it.each<[string, CombatForecast | null, number, number | undefined]>([
    ['a lethal attack', FORECAST, 0, -2],
    ['an attack that was all blocked', FORECAST, 2, 40],
    ['a seat nobody attacks', FORECAST, 1, undefined],
    ['no forecast', null, 0, undefined],
  ])('for %s', (_name, forecast, seat, lifeAfter) => {
    expect(playerOutlook(forecast, seat)?.life_after).toBe(lifeAfter)
  })
})

/** What the hooks give, printed, with a counter of how often each reader was drawn. */
const drawn = { readers: vi.fn(), quiet: vi.fn() }

function Readers() {
  const one = useCreatureOutlook(1)
  const three = useCreatureOutlook(3)
  const seat = usePlayerOutlook(0)
  const quiet = usePlayerOutlook(1)
  const preview = useBlockPreview()
  drawn.readers()
  return (
    <ul>
      <li data-testid="one">{one === null ? 'nothing' : `${one.damage}/${String(one.dies)}`}</li>
      <li data-testid="three">{three === null ? 'nothing' : 'something'}</li>
      <li data-testid="seat">{seat === null ? 'nothing' : `${seat.life}>${seat.life_after}`}</li>
      <li data-testid="quiet">{quiet === null ? 'nothing' : 'something'}</li>
      <li data-testid="preview">{preview === null ? 'none' : `${preview.creatures.length} creatures`}</li>
    </ul>
  )
}

function Quiet({ id, seat }: { id: ObjectId; seat: number }) {
  useCreatureOutlook(id)
  usePlayerOutlook(seat)
  drawn.quiet()
  return null
}

function mount() {
  const pending = createPendingCombatStore()
  drawn.readers.mockClear()
  drawn.quiet.mockClear()
  const table = renderTable({
    ui: (
      <PendingCombatContext value={pending}>
        <Readers />
        <Quiet id={77} seat={1} />
      </PendingCombatContext>
    ),
  })
  const setForecast = (forecast: CombatForecast | null): void =>
    table.update((state) => ({
      view: { ...must(state.view), combat_forecast: forecast },
    }))
  return { ...table, pending, setForecast }
}

const text = (id: string): string | null => screen.getByTestId(id).textContent

describe('the hooks over a table', () => {
  it('say nothing while the table has no forecast', () => {
    mount()
    expect(['one', 'three', 'seat', 'quiet'].map(text)).toEqual(['nothing', 'nothing', 'nothing', 'nothing'])
    expect(text('preview')).toBe('none')
  })

  it('show the table forecast: what happens to a creature, to a player, and the whole forecast', () => {
    const { setForecast } = mount()
    setForecast(FORECAST)
    expect(text('one')).toBe('4/true')
    expect(text('three')).toBe('nothing')
    expect(text('seat')).toBe('5>-2')
    expect(text('quiet')).toBe('nothing')
    expect(text('preview')).toBe('4 creatures')
  })

  it('follow the forecast when it changes and when it goes', () => {
    const { setForecast } = mount()
    setForecast(FORECAST)
    setForecast({ ...FORECAST, creatures: [{ object: 1, damage: 1, dies: false }] })
    expect(text('one')).toBe('1/false')
    expect(text('preview')).toBe('1 creatures')
    setForecast(null)
    expect(text('one')).toBe('nothing')
    expect(text('seat')).toBe('nothing')
    expect(text('preview')).toBe('none')
  })

  it('hide the forecast while blocks are being chosen, and bring it back when they are undone', () => {
    const { setForecast, pending } = mount()
    setForecast(FORECAST)
    act(() => pending.set({ blocks: new Map([[5, 1]]) }))
    expect(['one', 'seat', 'preview'].map(text)).toEqual(['nothing', 'nothing', 'none'])
    act(() => pending.clear())
    expect(['one', 'seat', 'preview'].map(text)).toEqual(['4/true', '5>-2', '4 creatures'])
  })

  it('are not hidden by pending attacks: those are not blocks', () => {
    const { setForecast, pending } = mount()
    setForecast(FORECAST)
    act(() => pending.set({ attacks: new Map([[5, { Player: 1 }]]) }))
    expect(text('one')).toBe('4/true')
  })

  it('do not redraw a reader the forecast says nothing of when a block is chosen', () => {
    const { setForecast, pending } = mount()
    setForecast(FORECAST)
    const before = drawn.quiet.mock.calls.length
    act(() => pending.set({ blocks: new Map([[5, 1]]) }))
    act(() =>
      pending.set({
        blocks: new Map([
          [5, 1],
          [6, 1],
        ]),
      }),
    )
    act(() => pending.clear())
    expect(drawn.quiet.mock.calls.length).toBe(before)
  })

  it('do not redraw a reader whose own entry did not change when the forecast around it does', () => {
    const { setForecast } = mount()
    setForecast(FORECAST)
    const before = drawn.quiet.mock.calls.length
    setForecast({ ...FORECAST, creatures: [...FORECAST.creatures, { object: 8, damage: 1, dies: false }] })
    expect(drawn.quiet.mock.calls.length).toBe(before)
  })
})
