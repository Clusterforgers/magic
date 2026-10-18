// Goal: what the table draws of a forecast is decided in one place: the table's own forecast while nothing is being chosen, and while a block or an attack is, only the engine's preview of exactly that choice for the pending decision (nothing until it arrives, nothing for an earlier choice or decision), per creature (only when there is damage to show or a death) and per player, delivered to each card and seat panel by itself, so that the many cards and panels the forecast does not mention are not redrawn when the forecast changes or the first block is chosen.
import { act, screen } from '@testing-library/react'
import { useEffect } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { previewKey } from '@/play/preview'
import type { PreviewState } from '@/play/preview'
import type { Answer, CombatForecast, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { CombatProviders } from './CombatProviders'
import { createDrawnForecastStore, useCreatureOutlook, usePlayerOutlook } from './drawn'
import {
  attacksAnswer,
  blocksAnswer,
  creatureOutlook,
  pendingAnswer,
  previewFor,
  useBlockPreview,
} from './forecast'
import { usePendingStore } from './pending'
import type { PendingCombatStore } from './pending'

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

/** The engine's preview of the one block the tests make: creature 5 blocks creature 1. */
const PREVIEWED: CombatForecast = {
  attackers: [],
  creatures: [{ object: 5, damage: 1, dies: false }],
  players: [],
}
const BLOCK_KEY = previewKey({ DeclareBlockers: [[5, 1]] })

describe('previewFor, the one place that decides which forecast is drawn', () => {
  const reply = (over: Partial<PreviewState> = {}): PreviewState => ({
    decision: 7,
    key: BLOCK_KEY,
    forecast: PREVIEWED,
    ...over,
  })

  it.each<
    [string, CombatForecast | null, string | null, PreviewState | null, number | null, CombatForecast | null]
  >([
    ['nothing chosen: the table is right about the answer', FORECAST, null, null, 7, FORECAST],
    ['nothing chosen and the table has none', null, null, reply(), 7, null],
    ['nothing chosen: a stored preview is no part of it', FORECAST, null, reply(), 7, FORECAST],
    ['a choice and no preview yet: the table knows nothing of it', FORECAST, BLOCK_KEY, null, 7, null],
    ['a choice and its preview', FORECAST, BLOCK_KEY, reply(), 7, PREVIEWED],
    ['a choice and the preview of another choice', FORECAST, BLOCK_KEY, reply({ key: 'other' }), 7, null],
    ['a choice and the preview of an earlier decision', FORECAST, BLOCK_KEY, reply({ decision: 6 }), 7, null],
    ['a choice and no decision pending', FORECAST, BLOCK_KEY, reply(), null, null],
    ['a choice whose preview is no forecast', FORECAST, BLOCK_KEY, reply({ forecast: null }), 7, null],
  ])('with %s', (_name, table, choice, preview, pending, expected) => {
    expect(previewFor(table, choice, preview, pending)).toBe(expected)
  })
})

describe('the answer a prompt would send for the choices it has made', () => {
  const blocks: ReadonlyMap<ObjectId, ObjectId> = new Map([
    [5, 1],
    [6, 1],
    [7, 2],
  ])
  const attacks = new Map([
    [3, { Player: 1 }],
    [4, { Planeswalker: 9 }],
  ])

  it.each<[string, Answer | null, Answer | null]>([
    ['no blocks', blocksAnswer(new Map()), null],
    [
      'blocks, in the order they were made',
      blocksAnswer(blocks),
      {
        DeclareBlockers: [
          [5, 1],
          [6, 1],
          [7, 2],
        ],
      },
    ],
    ['no attacks', attacksAnswer(new Map()), null],
    [
      'attacks, in the order they were chosen',
      attacksAnswer(attacks),
      {
        DeclareAttackers: [
          [3, { Player: 1 }],
          [4, { Planeswalker: 9 }],
        ],
      },
    ],
    ['nothing pending', pendingAnswer({ blocks: new Map(), attacks: new Map() }), null],
    ['pending blocks', pendingAnswer({ blocks, attacks: new Map() }), blocksAnswer(blocks)],
    ['pending attacks', pendingAnswer({ blocks: new Map(), attacks }), attacksAnswer(attacks)],
  ])('is %s', (_name, answer, expected) => {
    expect(answer).toEqual(expected)
  })

  it('has a key that tells apart the order in which blocks were made, which the engine keeps', () => {
    const one = previewKey(
      must(
        blocksAnswer(
          new Map([
            [5, 1],
            [6, 1],
          ]),
        ),
      ),
    )
    const other = previewKey(
      must(
        blocksAnswer(
          new Map([
            [6, 1],
            [5, 1],
          ]),
        ),
      ),
    )
    expect(one).not.toBe(other)
    expect(one).toBe(
      previewKey(
        must(
          blocksAnswer(
            new Map([
              [5, 1],
              [6, 1],
            ]),
          ),
        ),
      ),
    )
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

describe('the drawn forecast store', () => {
  it('holds the outlook of each creature and the forecast of each player the forecast mentions', () => {
    const store = createDrawnForecastStore()
    store.publish(FORECAST)
    expect(store.creature(1)).toEqual({ damage: 4, dies: true })
    expect(store.creature(3)).toBeNull()
    expect(store.creature(99)).toBeNull()
    expect(store.player(0)).toEqual(FORECAST.players[0])
    expect(store.player(1)).toBeNull()
    store.publish(null)
    expect(store.creature(1)).toBeNull()
    expect(store.player(0)).toBeNull()
  })

  it('tells only the listeners of an entry that changed, once, and none when the same forecast is published again', () => {
    const store = createDrawnForecastStore()
    const one = vi.fn()
    const two = vi.fn()
    const seat = vi.fn()
    const other = vi.fn()
    store.subscribeCreature(1, one)
    store.subscribeCreature(2, two)
    store.subscribePlayer(0, seat)
    store.subscribePlayer(1, other)
    const told = (): number[] => [one, two, seat, other].map((spy) => spy.mock.calls.length)
    store.publish(FORECAST)
    expect(told()).toEqual([1, 1, 1, 0])
    store.publish({ ...FORECAST, creatures: [...FORECAST.creatures] })
    expect(told()).toEqual([1, 1, 1, 0])
    store.publish({
      ...FORECAST,
      creatures: [
        { object: 1, damage: 4, dies: true },
        { object: 2, damage: 3, dies: false },
      ],
    })
    expect(told()).toEqual([1, 2, 1, 0])
    store.publish(null)
    expect(told()).toEqual([2, 3, 2, 0])
  })

  it('keeps the very same entry while it does not change, and stops telling a listener that left', () => {
    const store = createDrawnForecastStore()
    store.publish(FORECAST)
    const before = store.creature(1)
    const player = store.player(0)
    const listener = vi.fn()
    const stop = store.subscribeCreature(1, listener)
    store.publish({ ...FORECAST, creatures: FORECAST.creatures.map((creature) => ({ ...creature })) })
    expect(store.creature(1)).toBe(before)
    expect(store.player(0)).toBe(player)
    expect(listener).not.toHaveBeenCalled()
    stop()
    store.publish(null)
    expect(listener).not.toHaveBeenCalled()
  })
})

/** How often each reader was drawn. */
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

/** Hands the table's pending store to the test. */
function Grab({ onStore }: { onStore: (store: PendingCombatStore) => void }) {
  const store = usePendingStore()
  useEffect(() => {
    onStore(store)
  }, [store, onStore])
  return null
}

function mount() {
  let pending: PendingCombatStore | null = null
  drawn.readers.mockClear()
  drawn.quiet.mockClear()
  const table = renderTable({
    ui: (
      <CombatProviders>
        <Grab onStore={(store) => (pending = store)} />
        <Readers />
        <Quiet id={77} seat={1} />
      </CombatProviders>
    ),
  })
  const setForecast = (forecast: CombatForecast | null): void =>
    table.update((state) => ({ view: { ...must(state.view), combat_forecast: forecast } }))
  const setPreview = (preview: PreviewState | null): void => table.update(() => ({ preview }))
  /** The decision the table faces, which a preview must be for. */
  const decision = (): number => must(table.store.getState().pending).id
  return { ...table, pending: () => must(pending), setForecast, setPreview, decision }
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
    act(() => pending().set({ blocks: new Map([[5, 1]]) }))
    expect(['one', 'seat', 'preview'].map(text)).toEqual(['nothing', 'nothing', 'none'])
    act(() => pending().clear())
    expect(['one', 'seat', 'preview'].map(text)).toEqual(['4/true', '5>-2', '4 creatures'])
  })

  it('hide the table forecast while attacks are being chosen too: it knows nothing of them', () => {
    const { setForecast, pending } = mount()
    setForecast(FORECAST)
    act(() => pending().set({ attacks: new Map([[5, { Player: 1 }]]) }))
    expect(['one', 'seat', 'preview'].map(text)).toEqual(['nothing', 'nothing', 'none'])
    act(() => pending().clear())
    expect(text('one')).toBe('4/true')
  })

  it('show the preview of the blocks being chosen once the engine has answered, and only for those blocks', () => {
    const { setForecast, setPreview, pending, decision } = mount()
    setForecast(FORECAST)
    act(() => pending().set({ blocks: new Map([[5, 1]]) }))
    expect(text('preview')).toBe('none')
    // The engine answers: the creature the block hurts is marked, the table's own marks stay gone.
    setPreview({ decision: decision(), key: BLOCK_KEY, forecast: PREVIEWED })
    expect(text('preview')).toBe('1 creatures')
    expect(['one', 'seat'].map(text)).toEqual(['nothing', 'nothing'])
    // Another block makes the reply out of date until the engine answers for it.
    act(() =>
      pending().set({
        blocks: new Map([
          [5, 1],
          [6, 1],
        ]),
      }),
    )
    expect(text('preview')).toBe('none')
    // Taking the blocks back returns the table's forecast, whatever the store still holds.
    act(() => pending().clear())
    expect(text('preview')).toBe('4 creatures')
  })

  it('show the preview of the attack being chosen, with its numbers on the seat it hits', () => {
    const { setPreview, pending, decision } = mount()
    const attack = new Map([[3, { Player: 0 }]])
    act(() => pending().set({ attacks: attack }))
    expect(text('seat')).toBe('nothing')
    setPreview({ decision: decision(), key: previewKey(must(attacksAnswer(attack))), forecast: FORECAST })
    expect(text('seat')).toBe('5>-2')
    expect(text('one')).toBe('4/true')
  })

  it('ignore a preview that is for another decision, or that holds no forecast', () => {
    const { setPreview, pending, decision } = mount()
    act(() => pending().set({ blocks: new Map([[5, 1]]) }))
    setPreview({ decision: decision() + 1, key: BLOCK_KEY, forecast: PREVIEWED })
    expect(text('preview')).toBe('none')
    setPreview({ decision: decision(), key: BLOCK_KEY, forecast: null })
    expect(text('preview')).toBe('none')
    setPreview({ decision: decision(), key: BLOCK_KEY, forecast: PREVIEWED })
    expect(text('preview')).toBe('1 creatures')
  })

  it('do not redraw a reader the forecast says nothing of when a block is chosen', () => {
    const { setForecast, pending } = mount()
    setForecast(FORECAST)
    const before = drawn.quiet.mock.calls.length
    act(() => pending().set({ blocks: new Map([[5, 1]]) }))
    act(() =>
      pending().set({
        blocks: new Map([
          [5, 1],
          [6, 1],
        ]),
      }),
    )
    act(() => pending().clear())
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
