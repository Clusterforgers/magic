// Goal: over a real session the Priority prompt hands the whole turn over in a few messages: "End turn" sends one Yield and keeps the prompt until the server confirms, the server's Yielding replaces it with the quiet line, "Stop passing" sends one CancelYield, the next decision brings the prompt back with its buttons free again, and Space answers with the id of the decision on screen.
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PriorityOptions, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { primaryButton } from '@/test-utils/passControls'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })

const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: false,
  can_pass: true,
  ...over,
})
const decision = (id: number): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player: 0, kind: { Priority: options() } },
})

/** A live table of seat 0 that has been sent the fixture as a Snapshot. */
function mount() {
  const h = startSession()
  h.join(0, 2)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (...messages: ServerMessage[]): void => act(() => h.deliver(...messages))
  const indicator = (): HTMLElement | null => table.container.querySelector('[data-slot="yield-indicator"]')
  const secondary = (): HTMLElement =>
    must(table.container.querySelector<HTMLElement>('[data-slot="secondary-action"]'))
  const prompt = (): HTMLElement | null => table.container.querySelector('[data-decision="Priority"]')
  return { h, ...table, send, indicator, secondary, prompt }
}

describe('handing the turn over', () => {
  it('sends one Yield for "End turn", keeps the prompt until the server confirms, then shows the quiet line', async () => {
    const { h, send, user, secondary, prompt, indicator } = mount()
    send(decision(11))
    await user.dblClick(secondary())
    expect(h.sentOf('Yield')).toEqual([{ type: 'Yield', until: 'EndOfTurn' }])
    expect(h.sentOf('Answer')).toEqual([])
    expect(prompt(), 'the server has not said a word yet').not.toBeNull()
    expect(indicator()).toBeNull()

    send({ type: 'Yielding', until: 'EndOfTurn' })
    expect(prompt()).toBeNull()
    expect(await screen.findByText('Passing until the end of turn')).toBeInTheDocument()
    expect(indicator()).toHaveAttribute('data-yield', 'EndOfTurn')
    expect(h.state().pending).toBeNull()
    expect(h.state().yielding).toBe('EndOfTurn')
  })

  it('stops on request: one CancelYield, the line stays until the server confirms, and the next decision brings the prompt back', async () => {
    const { h, send, user, prompt, indicator, secondary } = mount()
    send(decision(11), { type: 'Yielding', until: 'MyTurn' })
    await user.click(await screen.findByRole('button', { name: 'Stop passing' }))
    expect(h.sentOf('CancelYield')).toEqual([{ type: 'CancelYield' }])
    expect(indicator()).not.toBeNull()

    send({ type: 'Yielding', until: null }, decision(12))
    expect(indicator()).toBeNull()
    expect(prompt()).not.toBeNull()
    expect(must(secondary())).toBeEnabled()
    expect(primaryButton()).toBeEnabled()
  })

  it('shows the prompt again with free buttons when a yield that ended at once is answered with a new decision', async () => {
    const { send, user, secondary, prompt } = mount()
    send(decision(11))
    await user.click(secondary())
    expect(secondary()).toBeDisabled()
    // The server did not pass (a yield that ends where it starts) and sends the same window again.
    send(decision(12))
    expect(prompt()).not.toBeNull()
    expect(secondary()).toBeEnabled()
    expect(primaryButton()).toBeEnabled()
  })

  it('goes on to the next prompt when the yield stopped for a spell: the notice, then the decision', () => {
    const { h, send, prompt, indicator } = mount()
    send({ type: 'Yielding', until: 'EndOfTurn' })
    expect(indicator()).not.toBeNull()
    send({ type: 'Yielding', until: null }, decision(13))
    expect(indicator()).toBeNull()
    expect(prompt()).not.toBeNull()
    expect(h.state().pending).toMatchObject({ id: 13 })
  })

  it('answers with the id of the decision on screen when Space is pressed', async () => {
    const { h, send, user } = mount()
    send(decision(21))
    await user.keyboard(' ')
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 21, answer: { Priority: 'Pass' } }])
    send(decision(22))
    await user.keyboard('{Enter}')
    expect(h.answeredIds()).toEqual([21, 22])
  })

  it('shows the line from the Welcome of a reconnect, with no prompt to answer', async () => {
    const { h, send, prompt, indicator } = mount()
    send(decision(11))
    act(() => h.net.last.fail())
    expect(prompt()).toBeNull()
    act(() => {
      h.timers.advance(250)
      h.open()
    })
    send(
      {
        type: 'Welcome',
        protocol_version: 1,
        game: 'g',
        player: 0,
        seats: 2,
        auto_pass: true,
        yielding: 'Resolve',
        stops: [],
        full_control: 'Off',
      },
      { type: 'Snapshot', seq: 2, state: fixture.view },
    )
    expect(await screen.findByText('Passing until the stack is empty')).toBeInTheDocument()
    expect(indicator()).toHaveAttribute('data-yield', 'Resolve')
    expect(prompt()).toBeNull()
  })
})
