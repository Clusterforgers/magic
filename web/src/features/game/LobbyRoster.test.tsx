// Goal: the lobby draws the server's roster as one row per seat (name, a connected dot, ready / not ready yet / not here, this seat marked), sums it up in a line, says each change once to a screen reader in a polite live region, prefers the roster to the lookup while the socket is open, and shows tables of 2 to 6 seats and long names without cutting anything off.
import { screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { renderGame } from '@/test-utils/gameHarness'
import { emptyTable, lobbyMessage, withSeats } from '@/test-utils/lobbyFrames'
import type { SeatFlags } from '@/test-utils/lobbyFrames'
import { lobbyBatchAt, lobbyStart } from '@/test-utils/transcripts'

afterEach(() => {
  vi.restoreAllMocks()
  window.sessionStorage.clear()
})

const rows = (): HTMLElement[] => within(screen.getByRole('list', { name: 'Seats' })).getAllByRole('listitem')
const summary = (): string => document.querySelector('[data-slot="lobby-summary"]')?.textContent ?? ''
const spoken = (): string => document.querySelector('[data-slot="lobby-announcement"]')?.textContent ?? ''
const stateOf = (row: HTMLElement): string | null => row.getAttribute('data-state')

/** A lobby page for `seat` of a table that has `count` seats, with the socket open and no roster yet. */
async function lobby(count: number, seat = 0, info?: unknown) {
  const h = await renderGame({
    script: info === undefined ? undefined : { game: () => jsonResponse(200, info) },
  })
  h.join(seat, count)
  await h.settle()
  return h
}

describe.each([2, 3, 4, 5, 6])('a table of %i seats', (count) => {
  // Every state in turn: ready and here, here and not ready, away and not ready, ready but away.
  const mixed = (): SeatFlags[] =>
    emptyTable(count).map(([name], seat): SeatFlags => {
      const state = [
        [true, true],
        [true, false],
        [false, false],
        [false, true],
      ][seat % 4]
      return [name, state?.[0] ?? false, state?.[1] ?? false]
    })

  it('has one row per seat in seat order, each in the state its two flags give', async () => {
    const h = await lobby(count)
    h.deliver(lobbyMessage(mixed()))
    expect(rows()).toHaveLength(count)
    const expected = ['ready', 'waiting', 'away', 'ready']
    expect(rows().map(stateOf)).toEqual(Array.from({ length: count }, (_, seat) => expected[seat % 4]))
    for (const [seat, row] of rows().entries()) {
      expect(row).toHaveAttribute('data-seat-row', String(seat))
      expect(row).toHaveTextContent(`Seat ${seat + 1}`)
      expect(row).toHaveTextContent(emptyTable(count)[seat]?.[0] ?? '')
    }
  })

  it('writes each state in words, so colour and icon are not needed', async () => {
    const h = await lobby(count)
    h.deliver(lobbyMessage(mixed()))
    const words = rows().map((row) => row.querySelector('[data-slot="seat-state"]')?.textContent)
    expect(words[0]).toBe('Ready')
    expect(words[1]).toBe('Not ready yet')
    if (count > 2) expect(words[2]).toBe('Not here')
    if (count > 3) expect(words[3]).toBe('Ready(away)')
    const dots = rows().map((row) => row.querySelector('[data-slot="connected-dot"]')?.textContent)
    expect(dots.slice(0, 2)).toEqual(['Connected', 'Connected'])
    if (count > 2) expect(dots[2]).toBe('Not connected')
  })

  it('marks this seat, wherever it sits, and no other', async () => {
    for (const seat of [0, count - 1]) {
      const h = await lobby(count, seat)
      h.deliver(lobbyMessage(emptyTable(count)))
      const marked = rows().filter((row) => within(row).queryByText('you') !== null)
      expect(marked.map((row) => row.getAttribute('data-seat-row'))).toEqual([String(seat)])
      h.unmount()
    }
  })

  it('sums the table up in one line', async () => {
    const h = await lobby(count)
    h.deliver(lobbyMessage(withSeats(emptyTable(count), { 0: [true, true], 1: [true, false] })))
    const waitingFor: Record<number, string> = {
      2: 'Bea to press Ready',
      3: 'Bea to press Ready and Cy to join',
      4: 'Bea to press Ready and Cy and Di to join',
      5: 'Bea to press Ready and Cy, Di and Ed to join',
      6: 'Bea to press Ready and 4 players to join',
    }
    expect(summary()).toBe(`1 of ${count} ready, waiting for ${waitingFor[count]}`)
  })
})

describe('what the roster does to the page', () => {
  const info = {
    started: false,
    seats: [
      { seat: 0, name: 'Ann' },
      { seat: 1, name: 'Bea' },
      { seat: 2, name: 'Cy' },
    ],
    connected: [1, 2],
    ready: [1, 2],
    result: null,
  }

  it('shows the roster instead of the lookup while the socket is open', async () => {
    const h = await lobby(3, 0, info)
    expect(rows().map(stateOf)).toEqual(['waiting', 'ready', 'ready'])
    h.deliver(lobbyMessage(withSeats(emptyTable(3), { 0: [true, true], 1: [true, false] })))
    expect(rows().map(stateOf)).toEqual(['ready', 'waiting', 'away'])
    expect(summary()).toBe('1 of 3 ready, waiting for Bea to press Ready and Cy to join')
  })

  it('goes back to the lookup when the socket drops, and to the roster when it is back', async () => {
    const h = await lobby(3, 0, info)
    h.deliver(lobbyMessage(emptyTable(3)))
    expect(rows().map(stateOf)).toEqual(['waiting', 'away', 'away'])
    h.fail()
    // The lookup is all there is while no connection can say more, and this seat itself is away.
    expect(rows().map(stateOf)).toEqual(['away', 'ready', 'ready'])
    await h.advance(250)
    h.join(0, 3)
    h.deliver(lobbyMessage(withSeats(emptyTable(3), { 0: [true, true] })))
    expect(rows().map(stateOf)).toEqual(['ready', 'away', 'away'])
  })

  it('draws the seats the roster lists even when the lookup has not come', async () => {
    let never: (response: Response) => void = () => undefined
    const h = await renderGame({
      script: { game: () => new Promise<Response>((resolve) => (never = resolve)) },
    })
    h.join(1, 3)
    h.deliver(lobbyMessage(withSeats(emptyTable(3), { 1: [true, true] })))
    expect(rows()).toHaveLength(3)
    expect(rows()[0]).toHaveTextContent('Ann')
    expect(summary()).toBe('1 of 3 ready, waiting for Ann and Cy to join')
    never(jsonResponse(200, info))
    await h.settle()
    expect(rows()[0]).toHaveTextContent('Ann')
  })

  it('leaves for the table when the game starts, with the roster gone', async () => {
    const h = await lobby(2)
    h.deliver(lobbyMessage(emptyTable(2)))
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    h.deliver(lobbyStart.raw[lobbyBatchAt])
    expect(screen.queryByRole('heading', { name: 'Waiting for players' })).toBeNull()
    expect(screen.getByRole('region', { name: 'Game table' })).toBeInTheDocument()
  })
})

describe('what a screen reader is told', () => {
  it('has a polite live region that is empty until something changes', async () => {
    const h = await lobby(3)
    const region = document.querySelector('[data-slot="lobby-announcement"]')
    expect(region).toHaveAttribute('aria-live', 'polite')
    expect(spoken()).toBe('')
    h.deliver(lobbyMessage(withSeats(emptyTable(3), { 1: [true, false] })))
    expect(spoken()).toBe('')
  })

  it('does not announce the step from the first answers to real data, whichever comes first', async () => {
    let answer: (response: Response) => void = () => undefined
    const h = await renderGame({
      script: { game: () => new Promise<Response>((resolve) => (answer = resolve)) },
    })
    h.join(0, 2)
    expect(rows().map(stateOf)).toEqual(['waiting', 'away'])
    answer(
      jsonResponse(200, {
        started: false,
        seats: [
          { seat: 0, name: 'Ann' },
          { seat: 1, name: 'Bea' },
        ],
        connected: [1],
        ready: [1],
        result: null,
      }),
    )
    await h.settle()
    expect(rows().map(stateOf)).toEqual(['waiting', 'ready'])
    expect(spoken()).toBe('')
  })

  it('says who pressed Ready and where things stand', async () => {
    const h = await lobby(3)
    h.deliver(lobbyMessage(withSeats(emptyTable(3), { 0: [true, false], 1: [true, false] })))
    h.deliver(lobbyMessage(withSeats(emptyTable(3), { 0: [true, false], 1: [true, true] })))
    expect(spoken()).toBe('Bea is ready. 1 of 3 ready, waiting for you to press Ready and Cy to join')
  })

  it('says who joined, who left and who took Ready back', async () => {
    const h = await lobby(3)
    const here = withSeats(emptyTable(3), { 0: [true, false], 1: [true, true] })
    h.deliver(lobbyMessage(here))
    h.deliver(lobbyMessage(withSeats(here, { 2: [true, false] })))
    expect(spoken()).toBe('Cy joined. 1 of 3 ready, waiting for you and Cy to press Ready')
    h.deliver(lobbyMessage(withSeats(here, { 2: [false, false] })))
    expect(spoken()).toBe('Cy left. 1 of 3 ready, waiting for you to press Ready and Cy to join')
    h.deliver(lobbyMessage(withSeats(here, { 1: [true, false] })))
    expect(spoken()).toBe(
      'Bea is not ready. 0 of 3 ready, waiting for you and Bea to press Ready and Cy to join',
    )
  })

  it('says "You are ready" for this seat, and again when it takes it back', async () => {
    const h = await lobby(2)
    const table = withSeats(emptyTable(2), { 0: [true, false], 1: [true, false] })
    h.deliver(lobbyMessage(table))
    h.deliver(lobbyMessage(withSeats(table, { 0: [true, true] })))
    expect(spoken()).toBe('You are ready. 1 of 2 ready, waiting for Bea to press Ready')
    h.deliver(lobbyMessage(table))
    expect(spoken()).toBe('You are not ready. 0 of 2 ready, waiting for you and Bea to press Ready')
  })

  it('keeps what it said when the same roster comes again', async () => {
    const h = await lobby(2)
    const table = withSeats(emptyTable(2), { 0: [true, false], 1: [true, false] })
    h.deliver(lobbyMessage(table))
    h.deliver(lobbyMessage(withSeats(table, { 1: [true, true] })))
    const said = spoken()
    h.deliver(lobbyMessage(withSeats(table, { 1: [true, true] })))
    expect(spoken()).toBe(said)
  })

  it('counts changes past the third instead of reading them all', async () => {
    const h = await lobby(6)
    h.deliver(lobbyMessage(emptyTable(6)))
    const four = { 1: [true, false], 2: [true, false], 3: [true, false], 4: [true, false] } as const
    h.deliver(lobbyMessage(withSeats(emptyTable(6), four)))
    expect(spoken()).toBe(
      'Bea joined, Cy joined, Di joined, 1 more change. ' +
        '0 of 6 ready, waiting for 5 players to press Ready and Flo to join',
    )
  })

  it('does not say that this page lost its own connection: the banner does', async () => {
    const h = await lobby(2)
    h.deliver(lobbyMessage(withSeats(emptyTable(2), { 0: [true, false], 1: [true, false] })))
    h.fail()
    expect(spoken()).toBe('')
  })
})

describe('long names', () => {
  const LONG = 'W'.repeat(32)

  it('are printed whole in the rows, wrapping instead of being cut off', async () => {
    const h = await lobby(6, 5)
    const names: SeatFlags[] = emptyTable(6).map(([, ...rest], seat): SeatFlags => [
      seat % 2 === 0 ? LONG : `${'Ünï '.repeat(7)}end`,
      ...rest,
    ])
    h.deliver(lobbyMessage(withSeats(names, { 0: [true, true] })))
    for (const [seat, [name]] of names.entries()) {
      const shown = rows()[seat]?.querySelector('[data-slot="seat-name"]')
      expect(shown?.textContent).toBe(name)
      expect(shown?.className).not.toMatch(/truncate|text-ellipsis|line-clamp/)
    }
  })

  it('are printed whole in the line under the rows too', async () => {
    const h = await lobby(3, 2)
    h.deliver(
      lobbyMessage([
        [LONG, true, false],
        ['Bea', true, false],
        ['Cy', true, false],
      ]),
    )
    expect(summary()).toBe(`0 of 3 ready, waiting for ${LONG}, Bea and you to press Ready`)
  })

  it('and a name that looks like markup is plain text', async () => {
    const h = await lobby(2)
    const sneaky = '<img src=x onerror=alert(1)>'
    h.deliver(
      lobbyMessage([
        [sneaky, true, false],
        ['Bea', true, false],
      ]),
    )
    expect(rows()[0]).toHaveTextContent(sneaky)
    expect(rows()[0]?.querySelector('img')).toBeNull()
  })
})
