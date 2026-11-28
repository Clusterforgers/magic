// Goal: the session keeps the server's word on a go-back request (the open request with its clock, how it ended), votes and cancels with one flat message each, drops its own seat from the request once it voted, and forgets the request on a new Welcome.
import { describe, expect, it, vi } from 'vitest'
import type { GoBackOffer, ServerMessage } from '@/protocol/generated'
import { startSession } from '@/test-utils/harness'

const main: GoBackOffer = { player: 1, step: 'Main1', active: 1, at: 20 }

const asked = (waiting: number[]): ServerMessage => ({
  type: 'GoBackAsked',
  offer: main,
  waiting,
  deadline_ms: 15_000,
})

function joined() {
  const h = startSession()
  h.join(0, 4)
  return h
}

describe('a go-back request', () => {
  it('keeps the open request with the time it arrived, and clears the last notice', () => {
    vi.spyOn(Date, 'now').mockReturnValue(1_000)
    const h = joined()
    h.deliver({ type: 'GoBackEnded', offer: main, outcome: { type: 'Withdrawn' } })
    h.deliver(asked([0, 2]))
    expect(h.state().goBackRequest).toEqual({ offer: main, waiting: [0, 2], deadlineMs: 15_000, at: 1_000 })
    expect(h.state().goBackNotice).toBeNull()
    vi.restoreAllMocks()
  })

  it.each([
    { type: 'Approved' } as const,
    { type: 'Declined', by: 2 } as const,
    { type: 'TimedOut', by: 2 } as const,
    { type: 'Withdrawn' } as const,
  ])('closes the request when it ended as $type and keeps how, until dismissed', (outcome) => {
    const h = joined()
    h.deliver(asked([0]))
    h.deliver({ type: 'GoBackEnded', offer: main, outcome })
    expect(h.state().goBackRequest).toBeNull()
    expect(h.state().goBackNotice).toEqual({ offer: main, outcome })
    h.session.dismissGoBackNotice()
    expect(h.state().goBackNotice).toBeNull()
  })

  it.each([true, false])('votes %s with one message and leaves the waiting list at once', (approve) => {
    const h = joined()
    h.deliver(asked([0, 2]))
    expect(h.session.voteGoBack(approve)).toBe(true)
    expect(h.sentOf('GoBackVote')).toEqual([{ type: 'GoBackVote', approve }])
    expect(h.state().goBackRequest?.waiting).toEqual([2])
  })

  it('cancels with one message and forgets the request on a new Welcome', () => {
    const h = joined()
    h.deliver(asked([2]))
    expect(h.session.cancelGoBack()).toBe(true)
    expect(h.sentOf('CancelGoBack')).toEqual([{ type: 'CancelGoBack' }])
    h.join(0, 4)
    expect(h.state().goBackRequest).toBeNull()
  })
})
