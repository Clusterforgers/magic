// Goal: the controls dock offers "Take back …" for the seat's offer and sends TakeBack; a seat the table waits on gets Allow and Decline with the time left, which vote; the asker and everyone else see that the table is asked; the end of a request and every refusal show in words and go by themselves; nothing can be pressed while the connection is down.
import { act, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PlayState, TakeBackNotice, TakeBackRequest } from '@/play/store'
import type { TakeBackOffer, TakeBackRefusal } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { TAKE_BACK_NOTICE_MS } from './TakeBackControls'

const offerOf = (kind: TakeBackOffer['kind'], name: string, player = 0): TakeBackOffer => ({
  player,
  kind,
  name,
  at: 10,
})

/** A table whose seat 0 is in the given take-back state, with spies on the session's take-back calls. */
function renderTakeBack(change: Partial<PlayState>, seats = 3) {
  const takeBack = vi.fn(() => true)
  const voteTakeBack = vi.fn(() => true)
  const dismissTakeBackNotice = vi.fn()
  const table = renderTable({
    seats,
    session: { takeBack, voteTakeBack, dismissTakeBackNotice },
    state: (state) => ({ ...state, phase: 'playing', status: 'open', ...change }),
  })
  const dock = (): HTMLElement => {
    const region = table.container.querySelector<HTMLElement>('[data-region="controls"]')
    if (region === null) throw new Error('no controls dock')
    return region
  }
  return { ...table, takeBack, voteTakeBack, dismissTakeBackNotice, dock }
}

const request = (offer: TakeBackOffer, waiting: number[]): TakeBackRequest => ({
  offer,
  waiting,
  deadlineMs: 15_000,
  at: Date.now(),
})

afterEach(() => {
  vi.useRealTimers()
})

describe('the Take back button', () => {
  it.each([
    [offerOf('Cast', 'Shock'), 'Take back casting Shock'],
    [offerOf('Activation', 'Prodigal Sorcerer'), 'Take back activating Prodigal Sorcerer'],
    [offerOf('Land', 'Forest'), 'Take back playing Forest'],
  ])('names what it undoes and sends TakeBack once', async (offer, words) => {
    const { takeBack, dock, user } = renderTakeBack({ takeBackOffer: offer })
    await user.click(within(dock()).getByRole('button', { name: words }))
    expect(takeBack).toHaveBeenCalledTimes(1)
  })

  it('is not there without an offer, once the game is over, or while a request is open', () => {
    const { update } = renderTakeBack({ takeBackOffer: null })
    expect(screen.queryByRole('button', { name: /^Take back/ })).toBeNull()
    update(() => ({ takeBackOffer: offerOf('Cast', 'Shock'), phase: 'over' }))
    expect(screen.queryByRole('button', { name: /^Take back/ })).toBeNull()
    update(() => ({ phase: 'playing', takeBackRequest: request(offerOf('Cast', 'Shock'), [1]) }))
    expect(screen.queryByRole('button', { name: /^Take back/ })).toBeNull()
  })

  it('cannot be pressed while the connection is down', async () => {
    const { takeBack, user } = renderTakeBack({
      takeBackOffer: offerOf('Cast', 'Shock'),
      status: 'reconnecting',
    })
    const button = screen.getByRole('button', { name: 'Take back casting Shock' })
    expect(button).toBeDisabled()
    await user.click(button)
    expect(takeBack).not.toHaveBeenCalled()
  })
})

describe('an open request', () => {
  it('asks a waiting seat to allow or decline, with the time left', async () => {
    const { voteTakeBack, user } = renderTakeBack({
      takeBackRequest: request(offerOf('Cast', 'Shock', 1), [0, 2]),
    })
    const prompt = screen.getByRole('group', { name: 'Take-back request' })
    expect(prompt).toHaveTextContent('Bo wants to take back casting Shock')
    expect(prompt).toHaveTextContent('15 s left')
    await user.click(within(prompt).getByRole('button', { name: 'Allow' }))
    await user.click(within(prompt).getByRole('button', { name: 'Decline' }))
    expect(voteTakeBack.mock.calls).toEqual([[true], [false]])
  })

  it('counts the time down', () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval', 'Date'] })
    renderTakeBack({ takeBackRequest: request(offerOf('Land', 'Forest', 1), [0]) })
    expect(screen.getByRole('group', { name: 'Take-back request' })).toHaveTextContent('15 s left')
    act(() => vi.advanceTimersByTime(4_000))
    expect(screen.getByRole('group', { name: 'Take-back request' })).toHaveTextContent('11 s left')
  })

  it('tells the asker it waits for the table, with no vote', () => {
    renderTakeBack({ takeBackRequest: request(offerOf('Cast', 'Shock'), [1, 2]) })
    expect(screen.getByText(/Taking back casting Shock\. Waiting for the table…/)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Allow' })).toBeNull()
  })

  it('tells a seat that already answered who asks, with no vote', () => {
    renderTakeBack({ takeBackRequest: request(offerOf('Activation', 'Prodigal Sorcerer', 1), [2]) })
    expect(
      screen.getByText(/Bo wants to take back activating Prodigal Sorcerer\. Waiting for the table…/),
    ).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Decline' })).toBeNull()
  })

  it('cannot be voted on while the connection is down', () => {
    renderTakeBack({ takeBackRequest: request(offerOf('Cast', 'Shock', 1), [0]), status: 'reconnecting' })
    expect(screen.getByRole('button', { name: 'Allow' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Decline' })).toBeDisabled()
  })
})

const shock = offerOf('Cast', 'Shock')

const NOTICES: [string, TakeBackNotice, string][] = [
  ['Approved', { offer: shock, outcome: { type: 'Approved' } }, 'Approved: Alice takes back casting Shock.'],
  [
    'Declined',
    { offer: shock, outcome: { type: 'Declined', by: 1 } },
    'Declined by Bo: casting Shock stays.',
  ],
  [
    'TimedOut',
    { offer: shock, outcome: { type: 'TimedOut', by: 2 } },
    'No answer from Cy: casting Shock stays.',
  ],
  [
    'Withdrawn',
    { offer: shock, outcome: { type: 'Withdrawn' } },
    'Withdrawn: the request to take back casting Shock no longer applies.',
  ],
  ...(
    [
      ['Off', 'Take-backs are off in this game.'],
      ['Nothing', 'There is nothing to take back.'],
      ['MovedOn', 'Too late to take that back: the game has moved on.'],
      ['OtherPlayerActed', 'Too late to take that back: another player has acted since.'],
      ['CardDrawn', 'Too late to take that back: a card was drawn since.'],
      ['CardRevealed', 'Too late to take that back: a card was revealed since.'],
      ['LibrarySearched', 'Too late to take that back: a library was searched since.'],
      ['LibraryShuffled', 'Too late to take that back: a library was shuffled since.'],
      ['Random', 'Too late to take that back: something random happened since.'],
    ] as [TakeBackRefusal, string][]
  ).map(([reason, words]): [string, TakeBackNotice, string] => [reason, { refused: reason }, words]),
]

describe('the notice', () => {
  it.each(NOTICES)('%s: says so as a status', (_name, notice, words) => {
    renderTakeBack({ takeBackNotice: notice })
    expect(screen.getByText(words)).toHaveAttribute('role', 'status')
  })

  it('goes when dismissed, and by itself after a while', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const { dismissTakeBackNotice } = renderTakeBack({ takeBackNotice: { refused: 'Nothing' } })
    act(() => screen.getByRole('button', { name: 'OK' }).click())
    expect(dismissTakeBackNotice).toHaveBeenCalledTimes(1)
    act(() => vi.advanceTimersByTime(TAKE_BACK_NOTICE_MS))
    expect(dismissTakeBackNotice).toHaveBeenCalledTimes(2)
  })
})
