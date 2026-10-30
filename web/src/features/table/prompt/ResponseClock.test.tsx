// Goal: the prompt of a decision on a clock shows the seconds left as text that counts down with the session's clock, a bar that drains over the time left (not drawn, with only the number, under a preference for less motion), says its sentence once to a screen reader, turns urgent in the last seconds and goes with the prompt at zero; a decision without a clock, or another decision's clock, shows nothing; the Commands panel shows the same countdown (with no spoken start of its own) while the seat has a clock.
import { act, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { DecisionKind, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { KINDS } from '@/test-utils/decisions'
import { startSession } from '@/test-utils/harness'
import { primaryButton } from '@/test-utils/passControls'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })

const decision = (id: number, kind: DecisionKind, deadlineMs?: number): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player: 0, kind },
  ...(deadlineMs === undefined ? {} : { deadline_ms: deadlineMs }),
})

/** A live table of seat 0 that has been sent the fixture as a Snapshot; nothing is asked yet. */
function mount() {
  const h = startSession()
  h.join(0, 2)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const wait = (ms: number): void => act(() => h.timers.advance(ms))
  const clock = (): HTMLElement | null =>
    table.container.querySelector('[data-slot="decision-line"] [data-slot="response-clock"]')
  const bar = (): HTMLElement | null =>
    table.container.querySelector('[data-slot="decision-line"] [data-slot="response-clock-bar"]')
  const words = (): string => must(clock()).querySelector('[role="timer"]')?.textContent ?? ''
  return { h, ...table, send, wait, clock, bar, words }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('the countdown', () => {
  it('says what happens at zero and how long is left, and counts down with the session clock', () => {
    const { send, wait, words } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    expect(words()).toBe('Passing for you in 10 s')
    wait(1_000)
    expect(words()).toBe('Passing for you in 9 s')
    wait(5_000)
    expect(words()).toBe('Passing for you in 4 s')
  })

  it('counts from the moment the decision arrived: a part of a second counts as the second it is in', () => {
    const { send, wait, words } = mount()
    send(decision(5, KINDS.Priority, 9_500))
    expect(words()).toBe('Passing for you in 10 s')
    wait(500)
    expect(words()).toBe('Passing for you in 9 s')
  })

  it('says what a blocker clock does at zero', () => {
    const { send, words } = mount()
    send(decision(5, KINDS.DeclareBlockers, 45_000))
    expect(words()).toBe('Declaring no blocks in 45 s')
  })

  it('turns urgent for the last three seconds, in words as well as colour', () => {
    const { send, wait, clock, words } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    wait(6_000)
    expect(clock()).not.toHaveAttribute('data-urgent')
    wait(1_000)
    expect(words()).toBe('Passing for you in 3 s')
    expect(clock()).toHaveAttribute('data-urgent', 'true')
    expect(must(clock()).querySelector('[role="timer"]')).toHaveClass('text-destructive')
  })

  it('has no buttons or other controls: it is only a read-out', () => {
    const { send, clock } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    expect(within(must(clock())).queryAllByRole('button')).toEqual([])
  })
})

describe('the bar', () => {
  it('drains over the time left, from a full bar, and keeps its duration while the seconds tick', () => {
    const { send, wait, bar } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    expect(must(bar()).style.animationDuration).toBe('10000ms')
    expect(must(bar()).style.getPropertyValue('--clock-from')).toBe('1')
    wait(4_000)
    expect(must(bar()).style.animationDuration).toBe('10000ms')
  })

  it('starts where the clock stands when the table is drawn late', () => {
    const h = startSession()
    h.join(0, 2)
    act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
    act(() => h.deliver(decision(5, KINDS.Priority, 10_000)))
    act(() => h.timers.advance(4_000))
    const table = renderLiveTable(h.session, fixtureLookup(fixture))
    const bar = must(table.container.querySelector<HTMLElement>('[data-slot="response-clock-bar"]'))
    expect(bar.style.animationDuration).toBe('6000ms')
    expect(Number(bar.style.getPropertyValue('--clock-from'))).toBeCloseTo(0.6)
  })

  it('is not drawn for a person who asks for less motion, and the number still counts', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      media: query,
      matches: true,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }))
    const { send, wait, bar, words } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    expect(bar()).toBeNull()
    expect(words()).toBe('Passing for you in 10 s')
    wait(2_000)
    expect(words()).toBe('Passing for you in 8 s')
  })

  it('is hidden from a screen reader, which hears the sentence instead', () => {
    const { send, bar } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    expect(must(bar()).parentElement).toHaveAttribute('aria-hidden', 'true')
  })
})

describe('what a screen reader hears', () => {
  it('one sentence when the prompt appears, which the countdown never repeats', () => {
    const { send, wait, clock } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    const status = (): string => must(clock()).querySelector('[role="status"]')?.textContent ?? ''
    expect(status()).toBe('You have 10 seconds to answer; after that the game passes for you.')
    wait(5_000)
    expect(status()).toBe('You have 10 seconds to answer; after that the game passes for you.')
  })

  it('says what blocking does', () => {
    const { send, clock } = mount()
    send(decision(5, KINDS.DeclareBlockers, 45_000))
    expect(must(clock()).querySelector('[role="status"]')).toHaveTextContent(
      'You have 45 seconds to answer; after that the game declares no blocks for you.',
    )
  })
})

describe('in the Commands panel', () => {
  const inPanel = (container: HTMLElement): HTMLElement | null =>
    container.querySelector('[data-slot="commands-panel"] [data-slot="commands-clock"]')

  it('shows the same countdown to a seat working there, counting down with the prompt', () => {
    const { send, wait, words, container } = mount()
    expect(inPanel(container)).toBeNull()
    send(decision(5, KINDS.Priority, 10_000))
    const panelWords = (): string => inPanel(container)?.querySelector('[role="timer"]')?.textContent ?? ''
    expect(panelWords()).toBe('Passing for you in 10 s')
    wait(7_000)
    expect(panelWords()).toBe('Passing for you in 3 s')
    expect(panelWords()).toBe(words())
    expect(inPanel(container)?.querySelector('[data-slot="response-clock"]')).toHaveAttribute(
      'data-urgent',
      'true',
    )
  })

  it('says nothing aloud of its own and has no controls: the prompt announces the clock', () => {
    const { send, container } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    expect(inPanel(container)?.querySelector('[role="status"]')).toBeNull()
    expect(within(must(inPanel(container))).queryAllByRole('button')).toEqual([])
    expect(
      container.querySelectorAll('[data-slot="decision-line"] [data-slot="response-clock"] [role="status"]'),
    ).toHaveLength(1)
  })

  it.each([
    [
      'a decision without a clock',
      (send: (message: ServerMessage) => void) => send(decision(5, KINDS.Priority)),
    ],
    ['no decision', () => undefined],
  ])('draws no clock for %s', (_what, ask) => {
    const { send, container } = mount()
    ask(send)
    expect(inPanel(container)).toBeNull()
  })

  it('goes with the prompt when the time is up', () => {
    const { send, wait, container } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    expect(inPanel(container)?.querySelector('[role="timer"]')).not.toBeNull()
    wait(10_000)
    expect(inPanel(container)).toBeNull()
  })
})

describe('when there is nothing to show', () => {
  it('draws nothing for a decision without a clock', () => {
    const { send, clock, container } = mount()
    send(decision(5, KINDS.Priority))
    expect(container.querySelector('[data-decision="Priority"]')).not.toBeNull()
    expect(clock()).toBeNull()
  })

  it('draws nothing for a decision that is another one than the clock counts', () => {
    const { h, send, clock } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    act(() => h.session.store.setState((state) => ({ clock: state.clock && { ...state.clock, id: 99 } })))
    expect(clock()).toBeNull()
  })

  it('goes with the prompt when the time is up, and leaves nothing sent', () => {
    const { h, send, wait, clock, container } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    wait(9_999)
    expect(clock()).not.toBeNull()
    wait(1)
    expect(clock()).toBeNull()
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(h.sentOf('Answer')).toEqual([])
  })

  it('goes when the seat answers, and the answer is sent once', async () => {
    const { h, send, clock, user } = mount()
    send(decision(5, KINDS.Priority, 10_000))
    await user.click(primaryButton())
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 5, answer: { Priority: 'Pass' } }])
    expect(clock()).toBeNull()
  })
})
