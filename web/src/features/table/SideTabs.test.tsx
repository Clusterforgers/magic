// Goal: a seat's Commands (first and on show at the start), the log, the chat and the stack are tabs of one side area that is a column from L up (always open), a 44 px rail at M that opens a panel over the table, and a sheet from the top bar below that; each tab says how much is new (unread chat of other seats and log lines worth reading, spells on the stack), nothing counts while its tab is on show, and what was on show when the reader looked away has been read.
import { act, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { appendChat, appendLog } from '@/play/store'
import type { PlayState } from '@/play/store'
import type { EventView } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import type { TableHarness } from '@/test-utils/tableHarness'

const WIDE = { width: 1600, height: 1000 }
const MEDIUM = { width: 1100, height: 800 }
const NARROW = { width: 800, height: 900 }
const PHONE = { width: 390, height: 844 }

const turn = (number: number): EventView => ({ Public: { TurnBegan: { number, active: 1 } } })
const pass = (player: number): EventView => ({ Public: { PriorityPassed: { player } } })

const logs = (table: TableHarness, ...events: EventView[]): void =>
  table.update((state: PlayState) => ({
    log: appendLog(state.log, (state.log.at(-1)?.seq ?? 0) + 1, events),
  }))
const said = (table: TableHarness, from: number, text: string): void =>
  table.update((state: PlayState) => ({ chat: appendChat(state.chat, { from, text }) }))

const side = () => screen.getByRole('complementary', { name: 'Commands, log, chat and stack' })
const tab = (name: string | RegExp) => screen.getByRole('tab', { name })

describe('a wide window: the column', () => {
  it('is always open, on the Commands of a seat, with Log, Chat and Stack a tab away', async () => {
    const { user } = renderTable({ seats: 4, options: { viewport: WIDE } })
    expect(side()).toHaveAttribute('data-side', 'column')
    expect(
      within(side())
        .getAllByRole('tab')
        .map((each) => each.textContent?.replace(/\d.*$/, '').trim()),
    ).toEqual(['Commands', 'Log', 'Chat', 'Stack'])
    expect(within(side()).getByRole('region', { name: 'Commands' })).toBeInTheDocument()
    await user.click(tab(/^Log/))
    expect(within(side()).getByRole('log', { name: 'Game log' })).toBeInTheDocument()
    expect(within(side()).queryByRole('log', { name: 'Chat' })).toBeNull()
    await user.click(tab(/^Chat/))
    expect(within(side()).getByRole('log', { name: 'Chat' })).toBeInTheDocument()
    expect(within(side()).queryByRole('log', { name: 'Game log' })).toBeNull()
    await user.click(tab(/^Stack/))
    expect(within(side()).getByRole('region', { name: 'Stack' })).toBeInTheDocument()
  })

  it('counts what others say while another tab is on show, and clears it when that tab opens', async () => {
    const table = renderTable({ seats: 2, options: { viewport: WIDE } })
    expect(tab('Chat')).toBeInTheDocument()
    said(table, 1, 'good luck')
    said(table, 1, 'have fun')
    expect(tab('Chat 2 unread')).toBeInTheDocument()
    await table.user.click(tab(/^Chat/))
    expect(tab('Chat')).toBeInTheDocument()
    // While the chat is on show new lines are read at once.
    said(table, 1, 'and again')
    expect(tab('Chat')).toBeInTheDocument()
    // Looking away takes that as read, and the next line is new.
    await table.user.click(tab(/^Log/))
    expect(tab('Chat')).toBeInTheDocument()
    said(table, 1, 'one more')
    expect(tab('Chat 1 unread')).toBeInTheDocument()
  })

  it('does not count what the reader said, or what was there before the table was drawn', () => {
    const table = renderTable({ seats: 2, options: { viewport: WIDE } })
    expect(table.store.getState().chat.length).toBeGreaterThan(0)
    expect(tab('Chat')).toBeInTheDocument()
    said(table, 0, 'my own line')
    expect(tab('Chat')).toBeInTheDocument()
  })

  it('counts the log lines worth reading on the Log tab once another tab is on show, and not the quiet ones', async () => {
    const table = renderTable({ seats: 2, options: { viewport: WIDE } })
    await table.user.click(tab(/^Chat/))
    logs(table, pass(0), pass(1), turn(6))
    expect(tab('Log 1 unread')).toBeInTheDocument()
    logs(table, turn(7), pass(0))
    expect(tab('Log 2 unread')).toBeInTheDocument()
    await table.user.click(tab(/^Log/))
    expect(tab('Log')).toBeInTheDocument()
  })

  it('says how many spells are on the stack, and nothing for an empty one', () => {
    const table = renderTable({ seats: 4, options: { viewport: WIDE } })
    expect(tab('Stack 3 on the stack')).toBeInTheDocument()
    table.update((state) => ({ view: state.view && { ...state.view, stack: { count: 0, cards: [] } } }))
    expect(tab('Stack')).toBeInTheDocument()
  })

  it('words a count of 100 or more as 99+', () => {
    const table = renderTable({ seats: 2, options: { viewport: WIDE } })
    for (let line = 0; line < 120; line += 1) said(table, 1, `line ${line}`)
    expect(within(tab(/^Chat/)).getByText('99+')).toBeInTheDocument()
  })
})

describe('a medium window: the rail', () => {
  it('is a 44 px column of four icon tabs, named with their counts, with the panel shut', () => {
    renderTable({ seats: 2, options: { viewport: MEDIUM } })
    expect(side()).toHaveAttribute('data-side', 'rail')
    // Three spells are on the stack at the start.
    expect(
      within(side())
        .getAllByRole('tab')
        .map((each) => each.getAttribute('aria-label')),
    ).toEqual(['Commands', 'Log', 'Chat', 'Stack, 3 on the stack'])
    expect(screen.queryByRole('log')).toBeNull()
  })

  it('opens a panel over the table from a tab, switches tabs without closing, and closes from the same tab', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport: MEDIUM } })
    await user.click(tab('Log'))
    expect(screen.getByRole('log', { name: 'Game log' })).toBeInTheDocument()
    await user.click(tab('Chat'))
    expect(screen.getByRole('log', { name: 'Chat' })).toBeInTheDocument()
    expect(screen.queryByRole('log', { name: 'Game log' })).toBeNull()
    await user.click(tab('Chat'))
    expect(screen.queryByRole('log')).toBeNull()
  })

  it('closes on Escape', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport: MEDIUM } })
    await user.click(tab('Log'))
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('log')).toBeNull()
  })

  it('counts on the icons while the panel is shut and clears the one that is opened', async () => {
    const table = renderTable({ seats: 2, options: { viewport: MEDIUM } })
    said(table, 1, 'are you there')
    expect(tab('Chat, 1 unread')).toBeInTheDocument()
    await table.user.click(tab(/^Chat/))
    expect(tab('Chat')).toBeInTheDocument()
    await table.user.click(tab('Chat'))
    said(table, 1, 'hello?')
    expect(tab('Chat, 1 unread')).toBeInTheDocument()
  })
})

describe.each([
  ['a tablet', NARROW],
  ['a phone', PHONE],
])('%s: the sheet', (_name, viewport) => {
  const button = () => screen.getByRole('button', { name: /^Commands, log, chat and stack/ })

  it('has no column, and a button in the top bar that opens a sheet with the same tabs', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport } })
    expect(screen.queryByRole('complementary', { name: 'Commands, log, chat and stack' })).toBeNull()
    expect(screen.queryByRole('dialog', { name: 'Commands, log, chat and stack' })).toBeNull()
    await user.click(button())
    const sheet = screen.getByRole('dialog', { name: 'Commands, log, chat and stack' })
    expect(within(sheet).getAllByRole('tab')).toHaveLength(4)
    expect(within(sheet).getByRole('region', { name: 'Commands' })).toBeInTheDocument()
    await user.click(within(sheet).getByRole('tab', { name: /^Log/ }))
    expect(within(sheet).getByRole('log', { name: 'Game log' })).toBeInTheDocument()
    await user.click(within(sheet).getByRole('tab', { name: /^Chat/ }))
    expect(within(sheet).getByRole('log', { name: 'Chat' })).toBeInTheDocument()
  })

  it('closes from its button, from Escape and from the way the top bar has it, and comes back as it was left', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport } })
    await user.click(button())
    await user.click(screen.getByRole('button', { name: 'Close' }))
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Commands, log, chat and stack' })).toBeNull(),
    )
    await user.click(button())
    await user.keyboard('{Escape}')
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Commands, log, chat and stack' })).toBeNull(),
    )
  })

  it('says on the button how much wants a look: new lines, chat of others and the spells on the stack', () => {
    const table = renderTable({ seats: 4, options: { viewport } })
    // Three spells are on the stack at the start.
    expect(button()).toHaveAccessibleName('Commands, log, chat and stack, 3 new')
    said(table, 2, 'hello')
    logs(table, turn(6))
    act(() => undefined)
    expect(button()).toHaveAccessibleName('Commands, log, chat and stack, 5 new')
  })

  it('counts nothing for the tab on show while the sheet is open, and everything else', async () => {
    const table = renderTable({ seats: 2, options: { viewport } })
    await table.user.click(button())
    await table.user.click(screen.getByRole('tab', { name: /^Log/ }))
    said(table, 1, 'while you read')
    logs(table, turn(9))
    const sheet = screen.getByRole('dialog', { name: 'Commands, log, chat and stack' })
    expect(within(sheet).getByRole('tab', { name: 'Chat 1 unread' })).toBeInTheDocument()
    expect(within(sheet).getByRole('tab', { name: 'Log' })).toBeInTheDocument()
  })
})
