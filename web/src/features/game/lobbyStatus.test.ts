// Goal: the lobby's headline names who it still waits for at any table from 2 to 12 seats, and the Ready button says in words why it is off (no deck with the selected deck's trouble, no connection, a request out) or what it does.
import { describe, expect, it } from 'vitest'
import { lobbyStatus, readyState } from './lobbyModel'
import type { LobbyRow, ReadyFacts } from './lobbyModel'

const seat = (n: number, over: Partial<LobbyRow> = {}): LobbyRow => ({
  seat: n,
  name: `P${n}`,
  named: true,
  you: n === 0,
  connected: true,
  ready: false,
  deck: null,
  ...over,
})

const table = (seats: number, here: number, ready: number): LobbyRow[] =>
  Array.from({ length: seats }, (_, n) => seat(n, { connected: n < here, ready: n < ready }))

describe('the headline', () => {
  it.each([2, 3, 4, 6, 8, 12])('waits for the players still to join at %i seats', (seats) => {
    expect(lobbyStatus(table(seats, 1, 0)).headline).toBe(
      `Waiting for ${seats - 1} more ${seats - 1 === 1 ? 'player' : 'players'}`,
    )
    expect(lobbyStatus(table(seats, seats - 1, 0)).headline).toBe('Waiting for 1 more player')
  })

  it.each([2, 3, 4, 6, 8, 12])('waits for everyone to get ready once all %i are here', (seats) => {
    const status = lobbyStatus(table(seats, seats, 1))
    expect(status.headline).toBe(
      `Waiting for ${seats - 1} ${seats - 1 === 1 ? 'player' : 'players'} to get ready`,
    )
    expect(status).toMatchObject({ ready: 1, seats, done: false })
  })

  it.each([2, 5, 12])('says the game starts when all %i seats are ready', (seats) => {
    expect(lobbyStatus(table(seats, seats, seats))).toEqual({
      headline: 'Everyone is ready: starting the game',
      ready: seats,
      seats,
      done: true,
    })
  })

  it('counts a ready seat whose client is away as ready, not as missing', () => {
    const rows = [seat(0, { ready: true }), seat(1, { ready: true, connected: false })]
    expect(lobbyStatus(rows).done).toBe(true)
  })

  it('has no headline for no seats', () => {
    expect(lobbyStatus([]).headline).toBe('')
  })
})

describe('the Ready button', () => {
  const base: ReadyFacts = { open: true, busy: false, ready: false, needsDeck: false, previewing: null }
  it.each<[string, Partial<ReadyFacts>, { label: string; disabled: boolean; hint: RegExp }]>([
    [
      'a deck and a connection',
      {},
      { label: 'Ready', disabled: false, hint: /^Press Ready when you are set to play\.$/ },
    ],
    [
      'no deck',
      { needsDeck: true },
      { label: 'Ready', disabled: true, hint: /^Ready needs a deck\. Choose one under Your deck\.$/ },
    ],
    [
      'no deck, one selected',
      { needsDeck: true, previewing: { name: 'Burn', blocked: false } },
      { label: 'Ready', disabled: true, hint: /Press Use this deck to take Burn\.$/ },
    ],
    [
      'no deck, the selected one blocked',
      { needsDeck: true, previewing: { name: 'Pauper', blocked: true } },
      {
        label: 'Ready',
        disabled: true,
        hint: /Choose one under Your deck\. Pauper cannot be taken at this table\.$/,
      },
    ],
    [
      'no connection',
      { open: false },
      { label: 'Ready', disabled: true, hint: /^Ready is back when the connection is\.$/ },
    ],
    ['a request out', { busy: true }, { label: 'Ready', disabled: true, hint: /Press Ready/ }],
    ['ready', { ready: true }, { label: 'Not ready', disabled: false, hint: /^You are ready\./ }],
    [
      'ready without a connection',
      { ready: true, open: false },
      { label: 'Not ready', disabled: true, hint: /^You are ready\./ },
    ],
  ])('with %s', (_what, facts, expected) => {
    const state = readyState({ ...base, ...facts })
    expect(state.label).toBe(expected.label)
    expect(state.disabled).toBe(expected.disabled)
    expect(state.hint).toMatch(expected.hint)
  })
})
