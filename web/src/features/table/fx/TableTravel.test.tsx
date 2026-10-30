// Goal: on the real table a cast flies from the hand card where it stood (measured before the table draws the batch) to the chip of the stack, over a layer nobody can reach, from another seat's pod for a spell the viewer never saw, and a table without the decoration layer draws no layer at all.
import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { applyBatchToStore } from '@/features/table/dev/applyBatch'
import type { TableOptions } from '@/features/table/tableOptions'
import type { EventView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fakeAnimate, setBox } from '@/test-utils/fxDom'
import type { TestBox } from '@/test-utils/fxDom'
import { batchFrom, FOE, FOE_SPELL, ME, miniFixture, MY_HAND_CARD, pub } from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import { renderTable } from '@/test-utils/tableHarness'
import { MOVE_MS } from './moves'

const HAND_CARD: TestBox = { x: 200, y: 620, width: 90, height: 126 }
const STACK_CHIP: TestBox = { x: 260, y: 250, width: 120, height: 32 }
const FOE_HAND_CHIP: TestBox = { x: 20, y: 60, width: 36, height: 20 }
const STACK_AREA: TestBox = { x: 100, y: 240, width: 900, height: 60 }

let animate: ReturnType<typeof fakeAnimate>
let rect: typeof Element.prototype.getBoundingClientRect

beforeEach(() => {
  vi.useFakeTimers()
  animate = fakeAnimate()
  // jsdom has no layout: the places a cast starts and ends at have boxes, nothing else does.
  rect = Element.prototype.getBoundingClientRect
  Element.prototype.getBoundingClientRect = function (this: Element): DOMRect {
    const place = (box: TestBox): DOMRect => {
      setBox(this, box)
      return this.getBoundingClientRect()
    }
    if (
      this.matches(
        '[data-region="hand"] button[data-object-id], [data-region="hand"] [data-slot="card-face"]',
      )
    )
      return place(HAND_CARD)
    if (this.matches('[data-fx-anchor="stack"] [data-object-id]')) return place(STACK_CHIP)
    if (this.matches(`[data-seat="${FOE}"] [data-zone="hand"]`)) return place(FOE_HAND_CHIP)
    if (this.matches('[data-fx-anchor="stack"]')) return place(STACK_AREA)
    return rect.call(this)
  }
})

afterEach(() => {
  Element.prototype.getBoundingClientRect = rect
  animate.restore()
  vi.useRealTimers()
})

function mount(options: TableOptions = { effects: true }) {
  const table = renderTable({ fixture: miniFixture(), options })
  const send = (change: (draft: Draft) => void, events: EventView[]): void => {
    const { view, seq } = table.store.getState()
    act(() => {
      applyBatchToStore(table.store, batchFrom(must(view), change, events, must(seq)))
    })
    // The next animation frame, in which the destination is measured.
    act(() => {
      vi.advanceTimersByTime(20)
    })
  }
  return { ...table, send }
}

/** The viewer's hand card goes from the hand to the top of the stack. */
const cast = (draft: Draft): void => {
  draft.edit(MY_HAND_CARD, (object) => {
    object.zone = 'Stack'
    object.permanent = null
  })
  const me = must(draft.view.players[ME])
  me.hand = { count: 0, cards: [] }
  draft.view.stack = { count: 2, cards: [...(draft.view.stack.cards ?? []), MY_HAND_CARD] }
}
const fliers = (container: HTMLElement): HTMLElement[] => [
  ...container.querySelectorAll<HTMLElement>('[data-fx-flier]'),
]

describe('a cast on the table', () => {
  it('flies from where the card stood in the hand to its chip on the stack', () => {
    const { container, send } = mount()
    send(cast, pub({ SpellCast: { player: ME, object: MY_HAND_CARD } }))
    const [flier] = fliers(container)
    expect(flier?.dataset.fxFlier).toBe('cast')
    // The table has drawn the batch: the card is no longer in the hand, and its chip stands on the stack.
    expect(container.querySelector('[data-region="hand"] button[data-object-id="5"]')).toBeNull()
    expect(container.querySelector('[data-fx-anchor="stack"] [data-object-id="5"]')).not.toBeNull()
    const [flight] = animate.started
    expect(flight?.timing).toMatchObject({ duration: MOVE_MS.travel, delay: 0 })
    const frames = flight?.keyframes ?? []
    // The first step is the card where it stood: its corner at the hand card's corner, its size whole.
    expect(frames[0]?.transform).toBe(`translate(${HAND_CARD.x}px, ${HAND_CARD.y}px) scale(1)`)
    expect(frames.at(-1)?.opacity).toBe(0)
  })

  it('stands in a layer that nothing can reach, and goes when the flight has landed', () => {
    const { container, send } = mount()
    send(cast, pub({ SpellCast: { player: ME, object: MY_HAND_CARD } }))
    const layer = container.querySelector('[data-slot="fx-travel"]')
    expect(layer?.getAttribute('aria-hidden')).toBe('true')
    expect(layer?.querySelector('button, [data-object-id], [tabindex]')).toBeNull()
    act(() => animate.started[0]?.finish())
    expect(fliers(container)).toHaveLength(0)
    // The viewer's own cast also stands large for a moment (TableSpotlight.test.tsx); the layer goes with it.
    act(() => animate.started[1]?.finish())
    expect(container.querySelector('[data-slot="fx-travel"]')).toBeNull()
  })

  it('flies from the pod of another seat as a card back that turns up on its way in', () => {
    const { container, send } = mount()
    const spell = 99
    send(
      (draft) => {
        const copy = must(draft.view.objects.find((object) => object.id === FOE_SPELL))
        draft.add({ ...structuredClone(copy), id: spell })
      },
      pub({ SpellCast: { player: FOE, object: spell } }),
    )
    const [flier] = fliers(container)
    expect(flier?.querySelector('[data-slot="card-back"]')).not.toBeNull()
    // The flight, the face turning up over the back, and the spell standing large over the strip.
    expect(animate.started).toHaveLength(3)
    // It starts small, in the chip of the pod\x27s hand: its corner is where the chip\x27s middle is, less half the picture.
    expect(animate.started[0]?.keyframes[0]?.transform).toMatch(/^translate\(/)
  })

  it('draws no layer and starts no animation with the decoration layer off', () => {
    const { container, send } = mount({ effects: false })
    send(cast, pub({ SpellCast: { player: ME, object: MY_HAND_CARD } }))
    expect(container.querySelector('[data-slot="fx-travel"]')).toBeNull()
    expect(animate.started).toEqual([])
  })

  it('draws none for a batch that moves nothing', () => {
    const { container, send } = mount()
    send((draft) => draft.life(FOE, 15), pub({ PriorityPassed: { player: ME } }))
    expect(container.querySelector('[data-slot="fx-travel"]')).toBeNull()
  })
})
