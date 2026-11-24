// Goal: in every hand layout (desktop fan, short dock, phone, a hand of rows) a hand card whose cast is too dear but whose Plainscycling is offered wears the ability's ring and names "Plainscycling {2}" on its foot (on the head that shows where a row lies over its foot), a click still cycles it, and its spoken name says so; a card that can be cast as well keeps the cast's look and the chooser.
import { act } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Answer, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { TableHarness } from '@/test-utils/tableHarness'
import { DEV_PROMPTS, DEV_TABLES } from './dev/prompts'
import { buildFixture } from './dev/fixture'
import type { Viewport } from './layout'

type Answers = ReturnType<typeof vi.fn<(given: Answer) => boolean>>

/** A layout: the window, the cards in hand and the room every element reports (jsdom has no layout; rows need one). */
interface Layout {
  viewport: Viewport
  hand?: number
  room?: { width: number; height: number }
}

afterEach(() => {
  vi.restoreAllMocks()
})

/** The cycling scene at `prompt` in `layout`, and its Angel of the Ruins. */
function cycling(prompt: string, layout: Layout): { h: TableHarness; answer: Answers; angel: ObjectId } {
  const { room } = layout
  if (room !== undefined) {
    vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(() => room.width)
    vi.spyOn(Element.prototype, 'clientHeight', 'get').mockImplementation(() => room.height)
  }
  const answer = vi.fn<(given: Answer) => boolean>(() => true)
  const built = buildFixture({ seats: 2, ...(layout.hand === undefined ? {} : { hand: layout.hand }) })
  const fixture = must(DEV_TABLES[prompt])(built)
  const pending = must(DEV_PROMPTS[prompt])(fixture)
  const h = renderTable({
    fixture,
    options: { viewport: layout.viewport },
    session: { answer },
    state: (state) => ({ ...state, pending }),
  })
  const objects = Object.values(h.store.getState().objectsById)
  const angel = must(objects.find((object) => object.name === 'Angel of the Ruins')).id
  return { h, answer, angel }
}

const button = (id: ObjectId): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-region="hand"] button[data-object-id="${id}"]`))
const cue = (id: ObjectId): Element | null => button(id).querySelector('[data-slot="card-ability"]')
const back = (id: ObjectId): boolean =>
  button(id).closest('[data-slot="hand-slot"]')?.hasAttribute('data-back') === true

const LAYOUTS: ReadonlyArray<[string, Layout, boolean]> = [
  ['the desktop fan', { viewport: { width: 1400, height: 900 } }, false],
  ['a short dock', { viewport: { width: 1000, height: 800 } }, false],
  ['a phone', { viewport: { width: 390, height: 844 } }, false],
  [
    'a hand of rows',
    { viewport: { width: 1400, height: 900 }, hand: 12, room: { width: 602, height: 161 } },
    true,
  ],
]

describe('a hand card whose only play is its ability', () => {
  it.each(LAYOUTS)('names it on its foot in %s', (_layout, layout, covered) => {
    const { h, answer, angel } = cycling('priority-cycling-only', layout)
    const card = button(angel)
    expect(card).toHaveAttribute('data-highlight', 'legal')
    expect(card).toHaveAttribute('data-play', 'ability')
    expect(cue(angel)?.textContent?.replace(/­/g, '')).toBe('Plainscycling2')
    expect(cue(angel)?.querySelector('[data-pip="2"]')).not.toBeNull()
    // Where a row lies over the foot, the cue moves to the head that shows (a style of the slot).
    expect(back(angel)).toBe(covered)
    expect(cue(angel)?.className).toMatch(/group-data-\[back\]\/slot:bottom-/)
    expect(card.getAttribute('aria-label')).toMatch(/Activate Plainscycling 2, not castable/)
    act(() => card.click())
    expect(answer.mock.calls).toEqual([[{ Priority: { Activate: { object: angel, ability: 1 } } }]])
    h.unmount()
  })
})

describe('a hand card that can be cast too', () => {
  it.each(LAYOUTS)('keeps the look of a cast in %s', (_layout, layout) => {
    const { h, answer, angel } = cycling('priority-cycling', layout)
    const card = button(angel)
    expect(card).toHaveAttribute('data-highlight', 'legal')
    expect(card).not.toHaveAttribute('data-play')
    expect(cue(angel)).toBeNull()
    act(() => card.click())
    expect(answer).not.toHaveBeenCalled()
    expect(document.querySelector('[data-slot="ability-chooser"]')).toHaveTextContent('Plainscycling')
    h.unmount()
  })
})
