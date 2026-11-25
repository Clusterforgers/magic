// Goal: the run page follows the run over its socket: each frame shows another member's vote or a new room at once with no timer; a frame or a read that predates a vote just sent never undoes it; a lost socket reads the run once in its place and a frame takes a failed read's error back; leaving the page stops following; without a socket the page reads once.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DelveFollowHandlers, DelvesApi } from '@/net/delves'
import { DelveError } from '@/net/delves'
import type { DelveView, Run } from '@/protocol/generated'
import { renderInRouter } from '@/test-utils/router'
import { DelvePage } from './DelvePage'
import { delveView, fakeDelves, hero } from './fakeDelves'
import { pickerDecks } from './fakePickerDecks'

const ana = hero()
const bo = hero({ name: 'Bo', user: 'u1' })
const bot = hero({ name: 'Bot', user: null, bot: 'Normal' })

function map(votes: Run['votes']): DelveView {
  return delveView({}, { status: 'Map', at: 0, cleared: [0], votes, heroes: [ana, bo, bot] })
}

/** A client whose socket the test drives: `push` sends a frame, `drop` closes it. */
function followed(base: DelvesApi) {
  let handlers: DelveFollowHandlers | null = null
  let follows = 0
  let stops = 0
  const client: DelvesApi = {
    ...base,
    follow: (_id, given) => {
      follows += 1
      handlers = given
      return () => {
        stops += 1
      }
    },
  }
  const socket = () => {
    if (handlers === null) throw new Error('the page follows no run')
    return handlers
  }
  return {
    client,
    push: (view: DelveView) => act(() => socket().view(view)),
    drop: () => act(() => socket().lost()),
    counts: () => ({ follows, stops }),
  }
}

async function page(client: DelvesApi) {
  return renderInRouter(<DelvePage id="d1" client={client} loadDecks={pickerDecks} find={async () => null} />)
}

describe('the run page follows the run', () => {
  it('shows each frame at once and reads nothing while the socket is open', async () => {
    let reads = 0
    const live = followed({ ...fakeDelves(map({})), get: async () => ((reads += 1), map({})) })
    await page(live.client)
    await live.push(map({}))
    const vote = await screen.findByRole('region', { name: 'Vote for the next room' })
    const event = within(vote).getByRole('button', { name: 'Vote for Event' })
    expect(event).not.toHaveTextContent('1 vote · Bo')
    await live.push(map({ 1: 2 }))
    expect(event).toHaveTextContent('1 vote · Bo')
    expect(reads).toBe(0)
    expect(live.counts()).toEqual({ follows: 1, stops: 0 })
  })

  it('keeps a vote just sent over a frame and a read that predate it', async () => {
    const before = map({})
    const after = map({ 0: 1 })
    let answerAct: (view: DelveView) => void = () => undefined
    let answerRead: (view: DelveView) => void = () => undefined
    const live = followed({
      ...fakeDelves(before),
      get: () => new Promise<DelveView>((resolve) => (answerRead = resolve)),
      act: () => new Promise<DelveView>((resolve) => (answerAct = resolve)),
    })
    await page(live.client)
    await live.push(before)
    const vote = await screen.findByRole('region', { name: 'Vote for the next room' })
    // The socket drops: its read leaves before the vote and answers after it.
    await live.drop()
    fireEvent.click(within(vote).getByRole('button', { name: 'Vote for Shop' }))
    await live.push(before)
    await act(async () => answerAct(after))
    expect(await within(vote).findByRole('button', { name: 'Your vote: Shop' })).toBeInTheDocument()
    await act(async () => answerRead(before))
    expect(within(vote).getByRole('button', { name: 'Your vote: Shop' })).toBeInTheDocument()
    await live.push(after)
    expect(within(vote).getByRole('button', { name: 'Your vote: Shop' })).toBeInTheDocument()
  })

  it('reads the run when the socket drops, and the next frame takes a failed read back', async () => {
    let fail = true
    const live = followed({
      ...fakeDelves(map({})),
      get: async () => {
        if (fail) throw new DelveError('network', 0, 'Could not reach the server.')
        return map({ 1: 2 })
      },
    })
    await page(live.client)
    await live.push(map({}))
    await live.drop()
    expect(await screen.findByText('Could not reach the server.')).toBeInTheDocument()
    await live.push(map({}))
    expect(screen.queryByText('Could not reach the server.')).toBeNull()
    fail = false
    await live.drop()
    const event = await screen.findByRole('button', { name: 'Vote for Event' })
    await act(async () => undefined)
    expect(event).toHaveTextContent('1 vote · Bo')
  })

  it('stops following when the page goes', async () => {
    const live = followed(fakeDelves(map({})))
    const { unmount } = await page(live.client)
    unmount()
    expect(live.counts()).toEqual({ follows: 1, stops: 1 })
  })

  it('reads the run once without a socket', async () => {
    let reads = 0
    await page({ ...fakeDelves(map({})), get: async () => ((reads += 1), map({})) })
    expect(await screen.findByRole('region', { name: 'Vote for the next room' })).toBeInTheDocument()
    expect(reads).toBe(1)
  })
})
