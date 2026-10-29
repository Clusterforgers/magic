// Goal: the stack panel lists spells top first with their controller and a clear empty state, the log prints one sentence for every event and follows the newest line unless scrolled up, and the chat shows what was said and sends with session.chat.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { MAX_CHAT_CHARS } from '@/play/session'
import { appendChat, appendLog } from '@/play/store'
import { indexById } from '@/play/view'
import type { EventView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { ChatPanel } from './ChatPanel'
import { LogPanel } from './LogPanel'
import { StackPanel } from './StackPanel'

describe('stack panel', () => {
  const stack = () => screen.getByRole('region', { name: 'Stack' })

  it('lists the spells top first, each with its controller', () => {
    renderTable({ seats: 4, ui: <StackPanel /> })
    expect(within(stack()).getByRole('heading', { name: 'Stack (3)' })).toBeInTheDocument()
    const items = within(within(stack()).getByRole('list', { name: 'Stack, top first' })).getAllByRole(
      'listitem',
    )
    expect(items.map((item) => item.querySelector('span.font-semibold')?.textContent)).toEqual([
      'Lightning Bolt',
      'Giant Growth',
      'Counterspell',
    ])
    expect(items[0]).toHaveTextContent('Instant · cast by Alice · resolves first')
    expect(items[1]).toHaveTextContent('Instant · cast by Bo')
    expect(items[2]).toHaveTextContent('Instant · cast by Cy')
  })

  it('names the top of the stack and the cost of each spell for a screen reader', () => {
    renderTable({ seats: 4, ui: <StackPanel /> })
    expect(
      screen.getByRole('button', {
        name: /^Top of the stack: Lightning Bolt, cost red, Instant, cast by Alice$/,
      }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Counterspell, cost blue, blue, Instant, cast by Cy' }),
    ).toBeInTheDocument()
  })

  it('says the stack is empty when it is', () => {
    renderTable({
      seats: 2,
      ui: <StackPanel />,
      state: (state) => {
        const view = clone(must(state.view))
        view.stack = { count: 0, cards: [] }
        return { ...state, view }
      },
    })
    expect(within(stack()).getByRole('heading', { name: 'Stack (0)' })).toBeInTheDocument()
    expect(stack()).toHaveTextContent('The stack is empty.')
    expect(within(stack()).queryByRole('list')).toBeNull()
  })

  it('puts a spell that is cast on top, and clears when it resolves', () => {
    const { update } = renderTable({ seats: 2, ui: <StackPanel /> })
    update((state) => {
      const view = clone(must(state.view))
      view.objects = [
        ...view.objects,
        card({
          id: 8000,
          owner: 1,
          controller: 1,
          zone: 'Stack',
          name: 'Shock',
          types: ['Instant'],
          permanent: null,
        }),
      ]
      view.stack = { count: 4, cards: [...(view.stack.cards ?? []), 8000] }
      return { view, objectsById: indexById(view.objects) }
    })
    const items = within(screen.getByRole('list', { name: 'Stack, top first' })).getAllByRole('listitem')
    expect(items).toHaveLength(4)
    expect(items[0]).toHaveTextContent('Shock')
    expect(items[0]).toHaveTextContent('resolves first')
    expect(items[1]).not.toHaveTextContent('resolves first')
    update((state) => {
      const view = clone(must(state.view))
      view.stack = { count: 0, cards: [] }
      return { view }
    })
    expect(stack()).toHaveTextContent('The stack is empty.')
  })

  it('is one tab stop that the arrow keys move along', async () => {
    const { user } = renderTable({ seats: 4, ui: <StackPanel /> })
    const buttons = within(stack()).getAllByRole('button')
    expect(buttons.map((button) => button.getAttribute('tabindex'))).toEqual(['0', '-1', '-1'])
    await user.tab()
    expect(buttons[0]).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(buttons[1]).toHaveFocus()
    expect(buttons.map((button) => button.getAttribute('tabindex'))).toEqual(['-1', '0', '-1'])
    await user.keyboard('{End}')
    expect(buttons[2]).toHaveFocus()
    await user.keyboard('{Home}')
    expect(buttons[0]).toHaveFocus()
  })
})

describe('log panel', () => {
  const log = () => screen.getByRole('log', { name: 'Game log' })

  it('prints a line for every event worth one and one for each round of passes', () => {
    const { store } = renderTable({ seats: 4, ui: <LogPanel /> })
    const events = store.getState().log
    expect(events.length).toBeGreaterThanOrEqual(50)
    const priority = events.filter(
      ({ event }) => 'Public' in event && /^Priority/.test(Object.keys(event.Public)[0] ?? ''),
    )
    expect(priority.length).toBeGreaterThan(0)
    // Every grant and every pass leaves the log, and a round of them comes back as one line.
    const lines = within(log()).getAllByRole('listitem')
    expect(lines.length).toBeGreaterThanOrEqual(events.length - priority.length)
    expect(lines.length).toBeLessThan(events.length)
    expect(log()).not.toHaveTextContent(/receives priority|passes priority/)
  })

  it('writes the events as sentences with the seats named by the lookup', () => {
    renderTable({ seats: 4, ui: <LogPanel /> })
    expect(log()).toHaveTextContent('Bo attacks Alice with')
    expect(log()).toHaveTextContent('Alice declares blockers:')
    expect(log()).toHaveTextContent('Alice casts Lightning Bolt')
    expect(log()).toHaveTextContent('Grizzly Bears dies from lethal damage')
    expect(log()).toHaveTextContent('Alice shuffles their library')
  })

  it('draws a round of passes quietly, with the passing seats as colour dots and named for a screen reader', () => {
    renderTable({ seats: 4, ui: <LogPanel /> })
    const passes = within(log()).getAllByText(/^All pass$/)
    expect(passes.length).toBeGreaterThan(0)
    for (const text of passes) {
      const line = must(text.closest('li'))
      expect(line).toHaveClass('text-muted-foreground')
      expect(line.querySelectorAll('[data-slot="seat-dot"]')).toHaveLength(4)
      expect(line).toHaveTextContent(/^All pass: .+ pass$/)
      for (const name of ['Alice', 'Bo', 'Cy', 'Dee']) expect(line).toHaveTextContent(name)
    }
    expect(within(log()).getByText('Alice casts Lightning Bolt')).not.toHaveClass('text-muted-foreground')
  })

  it('says so before anything has happened', () => {
    renderTable({ seats: 2, ui: <LogPanel />, state: (state) => ({ ...state, log: [] }) })
    expect(log()).toHaveTextContent('Nothing has happened yet.')
    expect(within(log()).queryByRole('list')).toBeNull()
  })

  it('adds the lines of a new batch at the end', () => {
    const { update } = renderTable({ seats: 2, ui: <LogPanel /> })
    const events: EventView[] = [
      { Public: { TurnBegan: { number: 6, active: 0 } } },
      { Hidden: { CardDrawn: { player: 1 } } },
    ]
    update((state) => ({ log: appendLog(state.log, state.log.length, events) }))
    const lines = within(log()).getAllByRole('listitem')
    expect(lines.at(-2)).toHaveTextContent("Turn 6 begins: Alice's turn")
    expect(lines.at(-1)).toHaveTextContent('Bo draws a card')
  })

  it('names a card by what the table shows now', () => {
    const { update, store } = renderTable({ seats: 2, ui: <LogPanel /> })
    const [first] = must(must(store.getState().view).battlefield.cards)
    update((state) => ({
      log: appendLog(state.log, state.log.length, [{ Public: { PermanentTapped: { object: must(first) } } }]),
    }))
    expect(within(log()).getAllByRole('listitem').at(-1)).toHaveTextContent(/becomes tapped$/)
    update((state) => ({
      log: appendLog(state.log, state.log.length, [{ Public: { PermanentTapped: { object: 424242 } } }]),
    }))
    expect(within(log()).getAllByRole('listitem').at(-1)).toHaveTextContent('a card becomes tapped')
  })

  it('can be scrolled with the keyboard', () => {
    renderTable({ seats: 2, ui: <LogPanel /> })
    expect(log()).toHaveAttribute('tabindex', '0')
  })

  it('follows the newest line, but stays where the reader scrolled to', () => {
    const { update } = renderTable({ seats: 2, ui: <LogPanel /> })
    const scroller = log()
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 1000 })
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 200 })
    const say = (text: string): void =>
      update((state) => ({
        log: appendLog(state.log, state.log.length, [
          { Public: { TurnBegan: { number: text.length, active: 0 } } },
        ]),
      }))
    say('a')
    expect(scroller.scrollTop).toBe(1000)
    // The reader scrolls up to read; the next line must not drag them down.
    scroller.scrollTop = 100
    act(() => {
      scroller.dispatchEvent(new Event('scroll'))
    })
    say('bb')
    expect(scroller.scrollTop).toBe(100)
    // Back at the bottom, it follows again.
    scroller.scrollTop = 790
    act(() => {
      scroller.dispatchEvent(new Event('scroll'))
    })
    say('ccc')
    expect(scroller.scrollTop).toBe(1000)
  })
})

describe('chat panel', () => {
  const chat = () => screen.getByRole('log', { name: 'Chat' })
  const input = () => screen.getByRole('textbox', { name: 'Message to the table' })

  it('shows what was said, by name, and "You" for this seat', () => {
    renderTable({ seats: 4, ui: <ChatPanel /> })
    expect(chat()).toHaveTextContent('Bo: Attacking with the whole squad.')
    expect(chat()).toHaveTextContent('You: I have a response.')
    expect(chat()).toHaveTextContent('Cy: Counterspell on the bolt, if it comes to that.')
  })

  it('says so before anyone has spoken', () => {
    renderTable({ seats: 2, ui: <ChatPanel />, state: (state) => ({ ...state, chat: [] }) })
    expect(chat()).toHaveTextContent('No messages yet.')
  })

  it('sends a line with session.chat on Enter and clears the box', async () => {
    const send = vi.fn(() => true)
    const { user } = renderTable({ seats: 2, ui: <ChatPanel />, session: { chat: send } })
    await user.type(input(), 'good game{Enter}')
    expect(send).toHaveBeenCalledExactlyOnceWith('good game')
    expect(input()).toHaveValue('')
  })

  it('sends with the Send button too', async () => {
    const send = vi.fn(() => true)
    const { user } = renderTable({ seats: 2, ui: <ChatPanel />, session: { chat: send } })
    await user.type(input(), 'hello')
    await user.click(screen.getByRole('button', { name: 'Send' }))
    expect(send).toHaveBeenCalledWith('hello')
  })

  it('keeps the text when the line could not be sent', async () => {
    const send = vi.fn(() => false)
    const { user } = renderTable({ seats: 2, ui: <ChatPanel />, session: { chat: send } })
    await user.type(input(), 'hello{Enter}')
    expect(send).toHaveBeenCalledOnce()
    expect(input()).toHaveValue('hello')
  })

  it('does not send a line with nothing in it', async () => {
    const send = vi.fn(() => true)
    const { user } = renderTable({ seats: 2, ui: <ChatPanel />, session: { chat: send } })
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled()
    await user.type(input(), '   {Enter}')
    expect(send).not.toHaveBeenCalled()
  })

  it('stops the box at the longest line the server takes', () => {
    renderTable({ seats: 2, ui: <ChatPanel /> })
    expect(input()).toHaveAttribute('maxlength', String(MAX_CHAT_CHARS))
  })

  it('shows a line another seat says as it arrives', () => {
    const { update } = renderTable({ seats: 2, ui: <ChatPanel /> })
    update((state) => ({ chat: appendChat(state.chat, { from: 1, text: 'nice block' }) }))
    expect(chat()).toHaveTextContent('Bo: nice block')
  })

  it('works through the fixture session: a sent line appears in the chat', async () => {
    const { user } = renderTable({ seats: 2, ui: <ChatPanel /> })
    await user.type(input(), 'thanks all{Enter}')
    expect(chat()).toHaveTextContent('You: thanks all')
  })
})
