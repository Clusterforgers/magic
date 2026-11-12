// Goal: the session keeps the server's word on take-backs (the seat's offer, the open request with its clock, how it ended or why it was refused), asks and votes with one flat message each, drops its own seat from the request once it voted, and forgets the offer and request when the game ends or a new Welcome comes.
import { describe, expect, it, vi } from 'vitest'
import type { ServerMessage, TakeBackOffer } from '@/protocol/generated'
import { startSession } from '@/test-utils/harness'

const shock: TakeBackOffer = { player: 0, kind: 'Cast', name: 'Shock', at: 12 }
const forest: TakeBackOffer = { player: 1, kind: 'Land', name: 'Forest', at: 20 }

const asked = (offer: TakeBackOffer, waiting: number[]): ServerMessage => ({
  type: 'TakeBackAsked',
  offer,
  waiting,
  deadline_ms: 15_000,
})

function joined() {
  const h = startSession()
  h.join(0, 3)
  return h
}

describe('what the server says', () => {
  it('keeps the offer, and null once there is none', () => {
    const h = joined()
    h.deliver({ type: 'TakeBackOffer', offer: shock })
    expect(h.state().takeBackOffer).toEqual(shock)
    h.deliver({ type: 'TakeBackOffer', offer: null })
    expect(h.state().takeBackOffer).toBeNull()
  })

  it('keeps the open request with the time it arrived, and clears the last notice', () => {
    vi.spyOn(Date, 'now').mockReturnValue(1_000)
    const h = joined()
    h.deliver({ type: 'TakeBackRefused', reason: 'CardDrawn' })
    h.deliver(asked(forest, [0, 2]))
    expect(h.state().takeBackRequest).toEqual({
      offer: forest,
      waiting: [0, 2],
      deadlineMs: 15_000,
      at: 1_000,
    })
    expect(h.state().takeBackNotice).toBeNull()
    vi.restoreAllMocks()
  })

  it.each([
    { type: 'Approved' } as const,
    { type: 'Declined', by: 2 } as const,
    { type: 'TimedOut', by: 2 } as const,
    { type: 'Withdrawn' } as const,
  ])('closes the request when it ended as $type and keeps how', (outcome) => {
    const h = joined()
    h.deliver(asked(forest, [0]))
    h.deliver({ type: 'TakeBackEnded', offer: forest, outcome })
    expect(h.state().takeBackRequest).toBeNull()
    expect(h.state().takeBackNotice).toEqual({ offer: forest, outcome })
  })

  it('keeps why the seat was refused, until it is dismissed', () => {
    const h = joined()
    h.deliver({ type: 'TakeBackRefused', reason: 'LibraryShuffled' })
    expect(h.state().takeBackNotice).toEqual({ refused: 'LibraryShuffled' })
    h.session.dismissTakeBackNotice()
    expect(h.state().takeBackNotice).toBeNull()
  })

  it('forgets the offer and the request when the game ends', () => {
    const h = joined()
    h.deliver({ type: 'TakeBackOffer', offer: shock })
    h.deliver(asked(shock, [1, 2]))
    h.deliver({ type: 'GameOver', result: 'Draw' })
    expect(h.state().takeBackOffer).toBeNull()
    expect(h.state().takeBackRequest).toBeNull()
  })
})

describe('asking and voting', () => {
  it('sends TakeBack as one flat message and changes nothing by itself', () => {
    const h = joined()
    h.deliver({ type: 'TakeBackOffer', offer: shock })
    expect(h.session.takeBack()).toBe(true)
    expect(h.sentOf('TakeBack')).toEqual([{ type: 'TakeBack' }])
    expect(h.state().takeBackOffer).toEqual(shock)
  })

  it.each([true, false])('sends TakeBackVote { approve: %s } and leaves the waiting seats', (approve) => {
    const h = joined()
    h.deliver(asked(forest, [0, 2]))
    expect(h.session.voteTakeBack(approve)).toBe(true)
    expect(h.sentOf('TakeBackVote')).toEqual([{ type: 'TakeBackVote', approve }])
    expect(h.state().takeBackRequest?.waiting).toEqual([2])
  })

  it('sends nothing while the socket is not open', () => {
    const h = startSession()
    expect(h.session.takeBack()).toBe(false)
    expect(h.session.voteTakeBack(true)).toBe(false)
  })
})
