// Goal: when an attachment changes host on the table, the combat overlay's layer draws one arrow of the attach kind from the old host (a creature or a player) to the new one, at once and for one second, in full motion and under reduced motion, and draws nothing for an attachment that merely stays, loses its host, enters attached or whose host changes hands, or while the decoration layer is off.
import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { applyBatchToStore } from '@/features/table/dev/applyBatch'
import { ARROW_MS } from '@/features/table/fx/arrows'
import type { TableOptions } from '@/features/table/tableOptions'
import type { EventView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import {
  batchFrom,
  FOE,
  FOE_IDOL,
  FOE_SPELL,
  miniFixture,
  MY_BEAR,
  MY_GIANT,
  pub,
} from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import { fakeRects } from '@/test-utils/rects'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from '../GameTable'
import type { Anchor } from './plan'
import { RectProviderContext } from './rects'

// The opponent's artifact stands for an Equipment; it starts on the viewer's Bear.
const GEAR = FOE_IDOL

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

function stubReducedMotion(): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    media: query,
    matches: query.includes('prefers-reduced-motion: reduce'),
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

function mount(options: TableOptions = {}, layout?: Parameters<typeof fakeRects>[0]) {
  const fixture = miniFixture()
  const gear = must(fixture.view.objects.find((object) => object.id === GEAR))
  gear.attached_to = MY_BEAR
  const rects = fakeRects(layout)
  const table = renderTable({
    fixture,
    options: { effects: true, ...options },
    ui: (
      <RectProviderContext value={rects.provider}>
        <GameTable />
      </RectProviderContext>
    ),
  })
  const send = (change: (draft: Draft) => void, events: EventView[]): void => {
    const { view, seq } = table.store.getState()
    act(() => {
      applyBatchToStore(table.store, batchFrom(must(view), change, events, must(seq)))
    })
  }
  const arrows = () =>
    [...table.container.querySelectorAll<SVGGElement>('[data-layer="lines"] g[data-kind="attach"]')].map(
      (group) => ({ from: group.getAttribute('data-from'), to: group.getAttribute('data-to') }),
    )
  const advance = (ms: number): void => {
    act(() => {
      vi.advanceTimersByTime(ms)
    })
  }
  return { ...table, send, arrows, advance }
}

const hang = (host: number | null, player?: number) => (draft: Draft) =>
  draft.edit(GEAR, (object) => {
    delete object.attached_to
    if (host !== null) object.attached_to = host
    if (player !== undefined) object.attached_player = player
  })

const attached = (to: number): EventView[] => pub({ Attached: { object: GEAR, to } })

describe('the arrow of an attachment that moved', () => {
  it('runs from the old host to the new one and stands for one second', () => {
    const { send, arrows, advance } = mount()
    expect(arrows()).toEqual([])
    send(hang(MY_GIANT), attached(MY_GIANT))
    expect(arrows()).toEqual([{ from: `card:${MY_BEAR}`, to: `card:${MY_GIANT}` }])
    advance(ARROW_MS - 1)
    expect(arrows()).toHaveLength(1)
    advance(1)
    expect(arrows()).toEqual([])
  })

  it('ends at the pod of a player the attachment moves onto', () => {
    const { send, arrows } = mount()
    send(hang(null, FOE), pub({ AttachedToPlayer: { object: GEAR, player: FOE } }))
    expect(arrows()).toEqual([{ from: `card:${MY_BEAR}`, to: `seat:${FOE}` }])
  })

  it('is drawn the same under reduced motion, where it is still', () => {
    stubReducedMotion()
    const { send, arrows, advance } = mount()
    send(hang(MY_GIANT), attached(MY_GIANT))
    expect(arrows()).toEqual([{ from: `card:${MY_BEAR}`, to: `card:${MY_GIANT}` }])
    advance(ARROW_MS)
    expect(arrows()).toEqual([])
  })

  it.each([
    ['it loses its host', hang(null), pub({ Unattached: { object: GEAR } })],
    [
      'its host changes hands',
      (draft: Draft) =>
        draft.edit(MY_BEAR, (o) => {
          o.controller = FOE
        }),
      [],
    ],
    [
      'a spell resolves',
      (draft: Draft) => draft.enter(FOE_SPELL),
      pub({ SpellResolved: { object: FOE_SPELL } }),
    ],
  ])('draws nothing when %s', (_name, change, events) => {
    const { send, arrows } = mount()
    send(change, events)
    expect(arrows()).toEqual([])
  })

  it('draws nothing while the decoration layer is off', () => {
    const { send, arrows } = mount({ effects: false })
    send(hang(MY_GIANT), attached(MY_GIANT))
    expect(arrows()).toEqual([])
  })

  it('is drawn between hosts that stand 5 px apart, where an edge to edge line has no room', () => {
    const next = (anchor: Anchor) =>
      anchor.kind === 'seat'
        ? { x: 0, y: 5000, width: 1000, height: 200 }
        : { x: anchor.id * 105, y: 100, width: 100, height: 140 }
    const { send, container } = mount({}, next)
    send(hang(MY_GIANT), attached(MY_GIANT))
    const path = container.querySelector('[data-layer="lines"] g[data-kind="attach"] .combat-line')
    expect(path?.getAttribute('d')).toMatch(/^M [\d.]+ [\d.]+ Q/)
  })

  it('draws the next arrow beside one that is still standing', () => {
    const { send, arrows } = mount()
    send(hang(MY_GIANT), attached(MY_GIANT))
    send(hang(MY_BEAR), attached(MY_BEAR))
    expect(arrows()).toEqual([
      { from: `card:${MY_BEAR}`, to: `card:${MY_GIANT}` },
      { from: `card:${MY_GIANT}`, to: `card:${MY_BEAR}` },
    ])
  })
})
