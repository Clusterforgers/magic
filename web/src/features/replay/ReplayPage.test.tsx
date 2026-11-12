// Goal: the replay page loads a finished game and walks it with first, back, play, forward and last, plays on by itself one step every PLAY_STEP_MS until the end, turns the table to any seat, sends the session cookie, never offers an action at the table, and says why when there is no replay or no connection.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { EventBatch, Replay } from '@/protocol/generated'
import { jsonResponse, fakeFetch } from '@/test-utils/fakeFetch'
import { renderInRouter } from '@/test-utils/router'
import { TEST_VIEWPORT } from '@/test-utils/tableHarness'
import { lobbyStart } from '@/test-utils/transcripts'
import { PLAY_STEP_MS, ReplayPage } from './ReplayPage'

const steps: EventBatch[] = lobbyStart.messages.flatMap((message) =>
  message.type === 'Events' ? [message] : [],
)
const replay: Replay = { seat: 0, names: ['Ann', 'Bo'], result: null, steps }
const GAME = '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'

afterEach(() => {
  vi.useRealTimers()
})

async function open(answer: () => Response | Promise<Response> = () => jsonResponse(200, replay)) {
  const server = fakeFetch(answer)
  await renderInRouter(<ReplayPage game={GAME} fetch={server.fetch} options={{ viewport: TEST_VIEWPORT }} />)
  return server
}

const at = () => screen.getByText(/^\d+ \/ \d+$/).textContent
const button = (name: string) => screen.getByRole('button', { name })

describe('the replay page', () => {
  it('loads the game with the cookie and starts at the first step from the account’s own seat', async () => {
    const server = await open()
    expect(await screen.findByRole('toolbar', { name: 'Replay' })).toBeInTheDocument()
    expect(server.calls[0]?.url).toBe(`/api/games/${GAME}/replay`)
    expect(server.calls[0]?.init?.credentials).toBe('include')
    expect(at()).toBe(`1 / ${steps.length}`)
    expect(screen.getByLabelText('Seen from')).toHaveValue('0')
    expect(screen.getByRole('option', { name: 'Seen from Ann (you)' })).toBeInTheDocument()
  })

  it('steps forward, back, to the end and to the start', async () => {
    await open()
    await screen.findByRole('toolbar', { name: 'Replay' })
    expect(button('First step')).toBeDisabled()
    fireEvent.click(button('Step forward'))
    fireEvent.click(button('Step forward'))
    expect(at()).toBe(`3 / ${steps.length}`)
    fireEvent.click(button('Step back'))
    expect(at()).toBe(`2 / ${steps.length}`)
    fireEvent.click(button('Last step'))
    expect(at()).toBe(`${steps.length} / ${steps.length}`)
    expect(button('Step forward')).toBeDisabled()
    expect(button('Play')).toBeDisabled()
    fireEvent.click(button('First step'))
    expect(at()).toBe(`1 / ${steps.length}`)
  })

  it('plays one step at a time and pauses', async () => {
    await open()
    await screen.findByRole('toolbar', { name: 'Replay' })
    vi.useFakeTimers()
    fireEvent.click(button('Play'))
    for (let n = 0; n < 3; n += 1) act(() => vi.advanceTimersByTime(PLAY_STEP_MS))
    expect(at()).toBe(`4 / ${steps.length}`)
    fireEvent.click(button('Pause'))
    act(() => vi.advanceTimersByTime(PLAY_STEP_MS * 3))
    expect(at()).toBe(`4 / ${steps.length}`)
  })

  it('turns the table to another seat', async () => {
    await open()
    await screen.findByRole('toolbar', { name: 'Replay' })
    fireEvent.change(screen.getByLabelText('Seen from'), { target: { value: '1' } })
    expect(screen.getByLabelText('Seen from')).toHaveValue('1')
    expect(at()).toBe(`1 / ${steps.length}`)
  })

  it.each([
    [() => jsonResponse(404, { error: 'not_found', message: 'no such resource' }), 'No replay for this game'],
    [() => jsonResponse(500, { error: 'internal', message: 'x' }), 'The replay could not be loaded'],
    [() => jsonResponse(200, { names: [], steps: [] }), 'The replay could not be loaded'],
    [() => Promise.reject(new TypeError('offline')), 'The replay could not be loaded'],
  ])('says why there is nothing to show (%#)', async (answer, title) => {
    await open(answer)
    expect(await screen.findByRole('heading', { name: title })).toBeInTheDocument()
  })
})
