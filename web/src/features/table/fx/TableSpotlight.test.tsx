// Goal: on the real table a spell cast by another seat stands large over the shared strip (its card, who casts what and at what) after its first leg, for the spotlight time, over a layer nobody can reach, and the viewer's own cast stands there too for 0.6 of that time in a lower-contrast frame without ever holding back the viewer's next answer; it is a still card under reduced motion, a newer batch replaces it, and a card the viewer cannot identify shows its back; marks show on the card a resolution touched and go on their own, and a death a resolving spell causes draws an arrow from the spell to the card.
import { act, fireEvent } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { applyBatchToStore } from '@/features/table/dev/applyBatch'
import type { TableOptions } from '@/features/table/tableOptions'
import type { Answer, EventView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fakeAnimate, setBox } from '@/test-utils/fxDom'
import type { TestBox } from '@/test-utils/fxDom'
import {
  batchFrom,
  FOE,
  FOE_SPELL,
  ME,
  miniFixture,
  MY_BEAR,
  MY_HAND_CARD,
  pub,
} from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import { primaryButton } from '@/test-utils/passControls'
import { renderTable } from '@/test-utils/tableHarness'
import { MOVE_MS } from './moves'

const HAND_CARD: TestBox = { x: 200, y: 620, width: 90, height: 126 }
const STACK_CHIP: TestBox = { x: 260, y: 250, width: 120, height: 32 }
const BEAR: TestBox = { x: 500, y: 420, width: 90, height: 126 }
const FOE_HAND_CHIP: TestBox = { x: 20, y: 60, width: 36, height: 20 }
const STACK_AREA: TestBox = { x: 100, y: 240, width: 900, height: 60 }

let animate: ReturnType<typeof fakeAnimate>
let rect: typeof Element.prototype.getBoundingClientRect

beforeEach(() => {
  vi.useFakeTimers()
  animate = fakeAnimate()
  rect = Element.prototype.getBoundingClientRect
  Element.prototype.getBoundingClientRect = function (this: Element): DOMRect {
    const place = (box: TestBox): DOMRect => {
      setBox(this, box)
      return this.getBoundingClientRect()
    }
    const is = (selector: string): boolean => this.matches(selector)
    if (is('[data-region="hand"] button[data-object-id], [data-region="hand"] [data-slot="card-face"]'))
      return place(HAND_CARD)
    if (is('[data-fx-anchor="stack"] [data-object-id]')) return place(STACK_CHIP)
    if (is(`[data-seat="${FOE}"] [data-zone="hand"]`)) return place(FOE_HAND_CHIP)
    if (is('[data-fx-anchor="stack"]')) return place(STACK_AREA)
    if (
      is(`button[data-object-id="${MY_BEAR}"], button[data-object-id="${MY_BEAR}"] [data-slot="card-face"]`)
    )
      return place(BEAR)
    return rect.call(this)
  }
})

afterEach(() => {
  Element.prototype.getBoundingClientRect = rect
  animate.restore()
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

function stubReducedMotion(): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    media: query,
    matches: query.includes('reduce'),
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

function mount(options: TableOptions = { effects: true }, answer?: (answer: Answer) => boolean) {
  const table = renderTable({
    fixture: miniFixture(),
    options,
    session: answer === undefined ? {} : { answer },
  })
  const send = (change: (draft: Draft) => void, events: EventView[]): void => {
    const { view, seq } = table.store.getState()
    act(() => {
      applyBatchToStore(table.store, batchFrom(must(view), change, events, must(seq)))
    })
    act(() => {
      vi.advanceTimersByTime(20)
    })
  }
  return { ...table, send }
}

const SPELL = 99

/** Another seat casts a copy of its creature spell from its hand and leaves it on the stack. */
const foeCasts =
  (over: (draft: Draft) => void = () => undefined) =>
  (draft: Draft): void => {
    const copy = must(draft.view.objects.find((object) => object.id === FOE_SPELL))
    draft.add({ ...structuredClone(copy), id: SPELL })
    over(draft)
  }
const foeCast = pub({ SpellCast: { player: FOE, object: SPELL } })

/** The viewer's hand card goes to the top of the stack. */
const myCast = (draft: Draft): void => {
  draft.edit(MY_HAND_CARD, (object) => {
    object.zone = 'Stack'
    object.permanent = null
  })
  must(draft.view.players[ME]).hand = { count: 0, cards: [] }
  draft.view.stack = { count: 2, cards: [...(draft.view.stack.cards ?? []), MY_HAND_CARD] }
}

const mineCast = pub({ SpellCast: { player: ME, object: MY_HAND_CARD } })

const spotlight = (container: HTMLElement): HTMLElement | null =>
  container.querySelector<HTMLElement>('[data-slot="fx-spotlight"]')
const spotlightAnimation = () => animate.started.find((a) => a.element.matches('[data-slot="fx-spotlight"]'))

describe('the spotlight of a cast', () => {
  it('stands over the strip for a cast by another seat, with the card, who casts it and what it aims at', () => {
    const { container, send } = mount()
    send(foeCasts(), foeCast)
    const shown = spotlight(container)
    expect(shown).not.toBeNull()
    expect(shown?.querySelector('[data-slot="card-face"]')).not.toBeNull()
    expect(shown?.querySelector('[data-slot="fx-spotlight-caption"]')?.textContent).toMatch(/casts/)
    // Above the strip, centred over it: its foot is eight pixels over the strip's top.
    const top = Number(shown?.style.top.replace('px', ''))
    expect(top).toBeGreaterThanOrEqual(8)
    expect(Number(shown?.style.left.replace('px', ''))).toBeGreaterThan(0)
  })

  it('comes after the first leg and lasts the spotlight time, in and out', () => {
    const { container, send } = mount()
    send(foeCasts(), foeCast)
    const started = spotlightAnimation()
    expect(started?.timing).toMatchObject({
      duration: MOVE_MS.spotlight,
      delay: MOVE_MS.travel,
      fill: 'both',
    })
    expect(started?.keyframes.map((frame) => frame.opacity)).toEqual([0, 1, 1, 0])
    act(() => started?.finish())
    expect(spotlight(container)).toBeNull()
  })

  it('stands in the layer nobody can reach', () => {
    const { container, send } = mount()
    send(foeCasts(), foeCast)
    const layer = container.querySelector('[data-slot="fx-travel"]')
    expect(layer?.getAttribute('aria-hidden')).toBe('true')
    expect(layer?.querySelector('button, a, input, [tabindex], [data-object-id]')).toBeNull()
  })

  it('stands for the viewer’s own cast that stays on the stack, shorter and as "You cast"', () => {
    const { container, send } = mount()
    send(myCast, mineCast)
    const shown = spotlight(container)
    expect(shown?.dataset.own).toBeDefined()
    expect(shown?.querySelector('[data-slot="card-face"]')).not.toBeNull()
    expect(shown?.querySelector('[data-slot="fx-spotlight-caption"]')?.textContent).toMatch(/^You cast/)
    expect(container.querySelector('[data-fx-flier="cast"]')).not.toBeNull()
    expect(spotlightAnimation()?.timing).toMatchObject({
      duration: 0.6 * MOVE_MS.spotlight,
      delay: MOVE_MS.travel,
      fill: 'both',
    })
    expect(spotlight(container)?.parentElement?.getAttribute('aria-hidden')).toBe('true')
  })

  it('leaves another seat’s spotlight without the own mark', () => {
    const { container, send } = mount()
    send(foeCasts(), foeCast)
    expect(spotlight(container)?.dataset.own).toBeUndefined()
  })

  it('never holds back the viewer’s next answer: the pass control answers while it stands', () => {
    const answer = vi.fn(() => true)
    const { container, send } = mount({ effects: true }, answer)
    send(myCast, mineCast)
    expect(spotlight(container)).not.toBeNull()
    fireEvent.click(primaryButton())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer).toHaveBeenCalledWith({ Priority: 'Pass' })
  })

  it('is replaced by a newer batch with moves, and leaves with the batch after its time', () => {
    const { container, send } = mount()
    send(myCast, mineCast)
    expect(spotlight(container)).not.toBeNull()
    send(foeCasts(), foeCast)
    expect(spotlight(container)?.dataset.own).toBeUndefined()
    expect(container.querySelectorAll('[data-slot="fx-spotlight"]')).toHaveLength(1)
  })

  it('is a still card of the same time under reduced motion, with nothing flying', () => {
    stubReducedMotion()
    const { container, send } = mount()
    send(myCast, mineCast)
    expect(spotlight(container)?.dataset.still).toBeDefined()
    expect(container.querySelector('[data-fx-flier]')).toBeNull()
    expect(spotlightAnimation()?.timing).toMatchObject({ duration: 0.6 * MOVE_MS.spotlight, delay: 0 })
    expect(spotlightAnimation()?.keyframes.every((frame) => frame.transform === undefined)).toBe(true)
  })

  it('never shows the back of a card of the viewer’s own: with no card to show it does not stand', () => {
    const { container, send } = mount()
    send((draft) => {
      myCast(draft)
      draft.edit(MY_HAND_CARD, (object) => {
        object.face_down = true
        object.name = ''
      })
    }, mineCast)
    expect(spotlight(container)).toBeNull()
  })

  it('stands for the viewer’s own cast that resolves in the same batch', () => {
    const { container, send } = mount()
    send(
      (draft) => {
        myCast(draft)
        draft.edit(MY_HAND_CARD, (object) => {
          object.zone = { Graveyard: ME }
        })
        draft.view.stack = { count: 1, cards: [FOE_SPELL] }
      },
      pub(
        { SpellCast: { player: ME, object: MY_HAND_CARD } },
        { ResolutionBegan: { object: MY_HAND_CARD } },
        { ResolutionEnded: { object: MY_HAND_CARD, outcome: 'Resolved' } },
      ),
    )
    expect(spotlight(container)).not.toBeNull()
  })

  it('is replaced by a newer batch that has moves', () => {
    const { container, send } = mount()
    send(foeCasts(), foeCast)
    expect(spotlight(container)).not.toBeNull()
    send(myCast, mineCast)
    expect(container.querySelectorAll('[data-slot="fx-spotlight"]')).toHaveLength(1)
    expect(spotlight(container)?.dataset.own).toBeDefined()
  })

  it('shows the back of a card the viewer cannot identify', () => {
    const { container, send } = mount()
    send(
      foeCasts((draft) => {
        draft.edit(SPELL, (object) => {
          object.face_down = true
          object.name = ''
        })
      }),
      foeCast,
    )
    const shown = spotlight(container)
    expect(shown?.querySelector('[data-slot="card-back"]')).not.toBeNull()
    expect(shown?.querySelector('[data-slot="card-face"]')).toBeNull()
    expect(shown?.textContent).toContain('a card')
  })

  it('is a still card under reduced motion: at once, for the same time, with nothing flying', () => {
    stubReducedMotion()
    const { container, send } = mount()
    send(foeCasts(), foeCast)
    expect(spotlight(container)?.dataset.still).toBeDefined()
    expect(container.querySelector('[data-fx-flier]')).toBeNull()
    const started = spotlightAnimation()
    expect(started?.timing).toMatchObject({ duration: MOVE_MS.spotlight, delay: 0 })
    expect(started?.keyframes.every((frame) => frame.transform === undefined)).toBe(true)
  })

  it('is not drawn with the decoration layer off', () => {
    const { container, send } = mount({ effects: false })
    send(foeCasts(), foeCast)
    expect(spotlight(container)).toBeNull()
  })
})

describe('the marks of a resolution', () => {
  const resolves = (draft: Draft): void => draft.enter(FOE_SPELL)
  const resolution = pub(
    { ResolutionBegan: { object: FOE_SPELL } },
    { SpellResolved: { object: FOE_SPELL } },
    { ResolutionEnded: { object: FOE_SPELL, outcome: 'Resolved' } },
  )

  it('shows a badge on the card the resolution touched and takes it off after the mark time', () => {
    const { container, send } = mount()
    send(resolves, resolution)
    const card = container.querySelector(`button[data-object-id="${FOE_SPELL}"]`)
    const badge = card?.querySelector('[data-fx-mark="resolved"]')
    expect(badge?.textContent).toBe('Resolved')
    expect(badge?.getAttribute('aria-hidden')).toBe('true')
    act(() => {
      vi.advanceTimersByTime(MOVE_MS.mark)
    })
    expect(container.querySelector('[data-fx-mark]')).toBeNull()
  })

  it('shows it under reduced motion too, as a badge that simply appears', () => {
    stubReducedMotion()
    const { container, send } = mount()
    send(resolves, resolution)
    expect(
      container.querySelector(`button[data-object-id="${FOE_SPELL}"] [data-fx-mark="resolved"]`),
    ).not.toBeNull()
  })

  it('shows none with the decoration layer off', () => {
    const { container, send } = mount({ effects: false })
    send(resolves, resolution)
    expect(container.querySelector('[data-fx-mark]')).toBeNull()
  })

  it('draws an arrow from the resolving spell to the creature it kills, measured before the table drew the batch', () => {
    const { container, send } = mount()
    send(
      (draft) => draft.die(MY_BEAR),
      pub(
        { ResolutionBegan: { object: FOE_SPELL } },
        { CreatureDied: { object: MY_BEAR, cause: 'LethalDamage' } },
      ),
    )
    const line = container.querySelector('g[data-kind="effect"]')
    expect(line).not.toBeNull()
    expect(container.querySelector('[data-layer="stack"] g[data-kind="effect"]')).toBe(line)
    act(() => {
      vi.advanceTimersByTime(MOVE_MS.arrow + MOVE_MS.mark)
    })
    expect(container.querySelector('g[data-kind="effect"]')).toBeNull()
  })
})
