// Goal: on the real table an attacker's combat damage plays as a strike: a picture of the creature lunges from where it stands at the life total of the player it damages and back, over a layer nobody can reach, the card itself hidden meanwhile, the life total's flash and number waiting for the hit; damage from anything else strikes nothing, and with the decoration layer off nothing plays.
import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { applyBatchToStore } from '@/features/table/dev/applyBatch'
import type { TableOptions } from '@/features/table/tableOptions'
import type { Event, EventView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fakeAnimate, setBox } from '@/test-utils/fxDom'
import type { TestBox } from '@/test-utils/fxDom'
import { batchFrom, FOE, ME, miniFixture, MY_BEAR, pub } from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import { renderTable } from '@/test-utils/tableHarness'
import { impactDelay, STRIKE_MS } from './strikes'

const BEAR: TestBox = { x: 300, y: 600, width: 100, height: 140 }
const FOE_LIFE: TestBox = { x: 320, y: 40, width: 60, height: 40 }

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
    if (this.matches(`[data-object-id="${MY_BEAR}"], [data-object-id="${MY_BEAR}"] [data-slot="card-face"]`))
      return place(BEAR)
    if (this.matches(`[data-seat="${FOE}"] [data-slot="life"]`)) return place(FOE_LIFE)
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
    act(() => {
      vi.advanceTimersByTime(20)
    })
  }
  return { ...table, send }
}

/** The bear attacks the foe and its 2 damage reach the foe's life (`hit` deals them). */
const fight = (draft: Draft): void => {
  draft.view.turn = { ...draft.view.turn, active: ME, step: 'CombatDamage' }
  draft.view.combat = {
    attackers: [{ object: MY_BEAR, target: { Player: FOE }, blocked: false, blockers: [] }],
    attackers_declared: true,
    blockers_declared: [FOE],
  }
  draft.life(FOE, must(draft.view.players[FOE]).life - 2)
}
const hit: Event = { DamageDealt: { source: MY_BEAR, target: { Player: FOE }, amount: 2 } }
const strikers = (container: HTMLElement): HTMLElement[] => [
  ...container.querySelectorAll<HTMLElement>('[data-fx-striker]'),
]
const foeLife = (container: HTMLElement): HTMLElement =>
  must(container.querySelector<HTMLElement>(`[data-seat="${FOE}"] [data-slot="life"]`))

describe('combat damage on the table', () => {
  it('strikes from the attacker to the life total and back, the card hidden meanwhile', () => {
    const { container, send } = mount()
    send(fight, pub({ StepBegan: { step: 'CombatDamage' } }, hit))
    const [striker] = strikers(container)
    expect(striker?.dataset.fxStriker).toBe(String(MY_BEAR))
    const lunge = must(animate.started.find((animation) => animation.element === striker))
    expect(lunge.timing).toMatchObject({ duration: STRIKE_MS, delay: 0 })
    expect(lunge.keyframes[0]?.transform).toBe(`translate(${BEAR.x}px, ${BEAR.y}px) scale(1)`)
    expect(lunge.keyframes.at(-1)?.transform).toBe(`translate(${BEAR.x}px, ${BEAR.y}px) scale(1)`)
    const hidden = animate.started.find(
      (animation) => animation.element.getAttribute('data-object-id') === String(MY_BEAR),
    )
    expect(hidden?.keyframes).toEqual([{ opacity: 0 }, { opacity: 0 }])
    const layer = container.querySelector('[data-slot="fx-strikes"]')
    expect(layer?.getAttribute('aria-hidden')).toBe('true')
    expect(layer?.querySelector('button, [data-object-id], [tabindex]')).toBeNull()
    act(() => lunge.finish())
    expect(container.querySelector('[data-slot="fx-strikes"]')).toBeNull()
  })

  it('flashes the life total as the strike lands, not before', () => {
    const { container, send } = mount()
    send(fight, pub({ StepBegan: { step: 'CombatDamage' } }, hit))
    expect(foeLife(container).dataset.fx).toBeUndefined()
    act(() => {
      vi.advanceTimersByTime(impactDelay(0))
    })
    expect(foeLife(container).dataset.fx).toBe('damaged')
  })

  it.each<[string, TableOptions, EventView[]]>([
    ['damage after priority', { effects: true }, pub({ PriorityGranted: { player: ME } }, hit)],
    ['the decoration layer off', { effects: false }, pub({ StepBegan: { step: 'CombatDamage' } }, hit)],
  ])('strikes nothing for %s', (_name, options, events) => {
    const { container, send } = mount(options)
    send(fight, events)
    expect(strikers(container)).toEqual([])
    expect(container.querySelector('[data-slot="fx-strikes"]')).toBeNull()
  })
})
