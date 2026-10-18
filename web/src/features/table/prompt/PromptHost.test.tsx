/** Goal: the host puts the prompt of this seat's decision in the centre strip and nothing else there, and `usePrompt` gives a prompt its decision, a once-only answer that cannot reach a newer decision, and a selection that starts empty for every decision id. */
import { act } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Answer, ObjectId } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import type { PromptApi, Selection } from './PromptContext'

// `Mulligan` is swapped for a probe that keeps what `usePrompt()` gave it, so the hook is tested on its own.
const probe = vi.hoisted(() => ({ api: null as PromptApi | null }))
vi.mock('./registry', async (importOriginal) => {
  const original = await importOriginal<typeof import('./registry')>()
  const { usePrompt } = await import('./PromptContext')
  const { createElement, useEffect } = await import('react')
  function Probe() {
    const api = usePrompt()
    useEffect(() => {
      probe.api = api
    })
    return createElement('p', { 'data-slot': 'probe' }, [...api.selection.ids].join(','))
  }
  return { PROMPTS: { ...original.PROMPTS, Mulligan: Probe } }
})

const decisionLine = (container: HTMLElement): HTMLElement =>
  must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
const mulligan = (id: number) => ({ id, decision: decisionFor('Mulligan') })

beforeEach(() => {
  probe.api = null
})

describe('the host', () => {
  it('keeps the read-only line while another seat holds priority', () => {
    const { container, update } = renderTable({ seats: 3 })
    update((state) => {
      const view = clone(must(state.view))
      view.priority = { holder: 2, passed: [] }
      return { view, pending: null }
    })
    const line = decisionLine(container)
    expect(line).toHaveTextContent('Waiting for Cy')
    expect(line).not.toHaveAttribute('data-decision')
    expect(container.querySelectorAll('[data-highlight]')).toHaveLength(0)
  })

  it('keeps the read-only line for a decision that names another seat', () => {
    const { container } = renderTable({
      seats: 3,
      state: (state) => ({ ...state, pending: { id: 2, decision: decisionFor('Mulligan', 1) } }),
    })
    expect(decisionLine(container)).not.toHaveAttribute('data-decision')
    expect(probe.api).toBeNull()
  })

  it('draws the prompt when the decision arrives and takes it away when it is answered', () => {
    const { container, update } = renderTable({ state: (state) => ({ ...state, pending: null }) })
    expect(container.querySelector('[data-decision]')).toBeNull()
    update(() => ({ pending: mulligan(3) }))
    expect(container.querySelector('[data-decision="Mulligan"]')).not.toBeNull()
    update(() => ({ pending: null }))
    expect(container.querySelector('[data-decision]')).toBeNull()
  })

  it('names the decision id, so a driver can tell two decisions of one kind apart', () => {
    const { container, update } = renderTable({ state: (state) => ({ ...state, pending: mulligan(3) }) })
    const id = () => container.querySelector('[data-decision]')?.getAttribute('data-decision-id')
    expect(id()).toBe('3')
    update(() => ({ pending: mulligan(4) }))
    expect(id()).toBe('4')
  })

  it('swaps one prompt for the next when the decision changes kind', () => {
    const { container, update } = renderTable({ state: (state) => ({ ...state, pending: mulligan(3) }) })
    expect(container.querySelector('[data-decision="Mulligan"]')).not.toBeNull()
    update(() => ({ pending: { id: 4, decision: decisionFor('Discard') } }))
    expect(container.querySelector('[data-decision="Mulligan"]')).toBeNull()
    expect(container.querySelector('[data-decision="Discard"]')).not.toBeNull()
  })
})

/** A table whose decision is the probe's (a Mulligan with id 7), over a session whose `answer` is `answer`. */
function renderProbe(answer: (answer: Answer) => boolean = () => true) {
  const sent = vi.fn(answer)
  const table = renderTable({
    state: (state) => ({ ...state, pending: mulligan(7) }),
    session: { answer: sent },
  })
  const api = (): PromptApi => must(probe.api)
  return { ...table, sent, api }
}

const KEEP: Answer = { Mulligan: 'Keep' }

describe('usePrompt', () => {
  it('gives the decision, its id and kind name', () => {
    const { api } = renderProbe()
    expect(api()).toMatchObject({ id: 7, name: 'Mulligan', decision: decisionFor('Mulligan') })
  })

  it('sends an answer once, however often it is called', () => {
    const { api, sent } = renderProbe()
    expect(api().answer(KEEP)).toBe(true)
    expect(api().answer(KEEP)).toBe(false)
    expect(api().answer({ Mulligan: 'Mulligan' })).toBe(false)
    expect(sent.mock.calls).toEqual([[KEEP]])
  })

  it('lets the player try again when the connection refused the answer', () => {
    let open = false
    const { api, sent } = renderProbe(() => open)
    expect(api().answer(KEEP)).toBe(false)
    open = true
    expect(api().answer(KEEP)).toBe(true)
    expect(api().answer(KEEP)).toBe(false)
    expect(sent).toHaveBeenCalledTimes(2)
  })

  it('refuses an answer built for a decision the server has replaced', () => {
    const { api, sent, update } = renderProbe()
    const stale = api().answer
    update(() => ({ pending: mulligan(8) }))
    expect(stale(KEEP)).toBe(false)
    expect(sent).not.toHaveBeenCalled()
    expect(api().answer(KEEP)).toBe(true)
    expect(sent).toHaveBeenCalledTimes(1)
  })

  it('refuses an answer once nothing is pending', () => {
    const { api, sent, update } = renderProbe()
    const answer = api().answer
    update(() => ({ pending: null }))
    expect(answer(KEEP)).toBe(false)
    expect(sent).not.toHaveBeenCalled()
  })

  it('throws outside a prompt', async () => {
    const { usePrompt } = await import('./PromptContext')
    const { renderHook } = await import('@testing-library/react')
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    expect(() => renderHook(() => usePrompt())).toThrow('usePrompt must be used inside a prompt')
    quiet.mockRestore()
  })
})

type Step = (selection: Selection) => void

describe('the selection', () => {
  const ids = (): ObjectId[] => [...must(probe.api).selection.ids]
  const run = (step: Step): void => {
    act(() => step(must(probe.api).selection))
  }

  it('starts empty and is shown by the prompt', () => {
    const { container } = renderProbe()
    expect(ids()).toEqual([])
    expect(container.querySelector('[data-slot="probe"]')).toHaveTextContent('')
  })

  const table: [string, Step[], ObjectId[]][] = [
    ['toggles cards on', [(s) => s.toggle(4), (s) => s.toggle(9)], [4, 9]],
    ['toggles a card off again', [(s) => s.toggle(4), (s) => s.toggle(9), (s) => s.toggle(4)], [9]],
    ['keeps the order of choice', [(s) => s.toggle(9), (s) => s.toggle(4)], [9, 4]],
    ['replaces with set', [(s) => s.toggle(4), (s) => s.set([7, 8])], [7, 8]],
    ['sets nothing', [(s) => s.toggle(4), (s) => s.set([])], []],
    ['clears', [(s) => s.toggle(4), (s) => s.toggle(9), (s) => s.clear()], []],
  ]
  it.each(table)('%s', (_name, steps, expected) => {
    const { container } = renderProbe()
    for (const step of steps) run(step)
    expect(ids()).toEqual(expected)
    expect(container.querySelector('[data-slot="probe"]')).toHaveTextContent(expected.join(','))
    for (const id of expected) expect(must(probe.api).selection.has(id)).toBe(true)
    expect(must(probe.api).selection.has(12345)).toBe(false)
  })

  it('is empty again when the decision id changes', () => {
    const { update } = renderProbe()
    run((s) => s.set([4, 9]))
    expect(ids()).toEqual([4, 9])
    update(() => ({ pending: mulligan(8) }))
    expect(ids()).toEqual([])
  })

  it('is kept while the same decision is redrawn', () => {
    const { update } = renderProbe()
    run((s) => s.set([4]))
    update(() => ({ chat: [{ from: 1, text: 'hi' }] }))
    expect(ids()).toEqual([4])
  })

  it('is empty after the decision went away and came back', () => {
    const { update } = renderProbe()
    run((s) => s.set([4]))
    update(() => ({ pending: null }))
    update(() => ({ pending: mulligan(7) }))
    expect(ids()).toEqual([])
  })
})
