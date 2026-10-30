// Goal: a manual action is answered once, in order: the batch that starts with it says it was taken, a refusal goes to the oldest action still in flight and not to a combat preview or a decision answer sent before it, a refusal with nobody listening stays the table notice, and a lost connection says so.
import { describe, expect, it, vi } from 'vitest'
import type { Answer, EventView, ManualAction } from '@/protocol/generated'
import { batchOf, must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { lateJoin } from '@/test-utils/transcripts'
import type { ManualResult } from './manualLedger'

const TAP: ManualAction = { Tap: { object: 3 } }
const DRAW: ManualAction = { Draw: { count: 1 } }
const BLOCK: Answer = { DeclareBlockers: [[63, 30]] }

/** A seat in a running game with a block decision (id 12) pending. */
function playing() {
  const h = startSession()
  h.join()
  h.deliver(lateJoin.raw[1])
  h.deliver({
    type: 'Decision',
    id: 12,
    decision: { player: 0, kind: { DeclareBlockers: { blockers: [{ object: 63, can_block: [30] }] } } },
  })
  const seat = must(h.state().seat)
  /** The batch that starts with `action` taken by `by`. */
  const taken = (action: ManualAction, by = seat) => {
    const event: EventView = { Public: { ManualAction: { player: by, action } } }
    return {
      type: 'Events',
      ...batchOf(must(h.state().view), { from_seq: h.state().seq ?? 0, events: [event] }),
    }
  }
  const refuse = (code: 'IllegalAnswer' | 'BadMessage', message: string) => ({ type: 'Error', code, message })
  return { ...h, seat, taken, refuse }
}

/** What a reply was told, in order. */
function replies() {
  const heard: ManualResult[] = []
  return { heard, reply: (result: ManualResult) => heard.push(result) }
}

describe('a manual action that was taken', () => {
  it('is told ok when the batch that starts with it comes back, once', () => {
    const h = playing()
    const { heard, reply } = replies()
    expect(h.session.manual(TAP, reply)).toBe(true)
    expect(heard).toEqual([])
    h.deliver(h.taken(TAP))
    expect(heard).toEqual([{ ok: true }])
    h.deliver(h.taken(DRAW))
    expect(heard).toHaveLength(1)
  })

  it('is not settled by the manual action of another seat', () => {
    const h = playing()
    const { heard, reply } = replies()
    h.session.manual(TAP, reply)
    h.deliver(h.taken(TAP, h.seat + 1))
    expect(heard).toEqual([])
  })

  it('settles in the order it was asked', () => {
    const h = playing()
    const first = replies()
    const second = replies()
    h.session.manual(TAP, first.reply)
    h.session.manual(DRAW, second.reply)
    h.deliver(h.taken(TAP))
    expect(first.heard).toEqual([{ ok: true }])
    expect(second.heard).toEqual([])
    h.deliver(h.taken(DRAW))
    expect(second.heard).toEqual([{ ok: true }])
  })

  it('is taken when a later answer proves the server had passed it', () => {
    const h = playing()
    const { heard, reply } = replies()
    h.session.manual(TAP, reply)
    expect(h.session.preview(12, BLOCK)).toBe(true)
    h.deliver({ type: 'Forecast', decision: 12, forecast: null })
    expect(heard).toEqual([{ ok: true }])
  })
})

describe('a refused manual action', () => {
  it.each(['IllegalAnswer', 'BadMessage'] as const)(
    'tells its sender the %s reason and leaves the table notice alone',
    (code) => {
      const h = playing()
      const { heard, reply } = replies()
      h.session.manual(TAP, reply)
      h.deliver(h.refuse(code, 'Grizzly Bears is already tapped'))
      expect(heard).toEqual([{ ok: false, reason: 'Grizzly Bears is already tapped' }])
      expect(h.state().notice).toBeNull()
      expect(h.state().pending).toMatchObject({ id: 12 })
    },
  )

  it('needs no resync, since it changed nothing', () => {
    const h = playing()
    h.session.manual(TAP, replies().reply)
    h.deliver(h.refuse('IllegalAnswer', 'already tapped'))
    expect(h.sentOf('Resync')).toEqual([])
  })

  it.each([
    ['IllegalAnswer', /not allowed: already tapped/, 1],
    ['BadMessage', /did not accept that: already tapped/, 0],
  ] as const)('with nobody listening is the %s notice as before', (code, notice, resyncs) => {
    const h = playing()
    h.session.manual(TAP)
    h.deliver(h.refuse(code, 'already tapped'))
    expect(h.state().notice).toMatch(notice)
    expect(h.sentOf('Resync')).toHaveLength(resyncs)
  })

  it('goes to the oldest of two actions in flight and the next refusal to the other', () => {
    const h = playing()
    const first = replies()
    const second = replies()
    h.session.manual(TAP, first.reply)
    h.session.manual(DRAW, second.reply)
    h.deliver(h.refuse('IllegalAnswer', 'one'), h.refuse('IllegalAnswer', 'two'))
    expect(first.heard).toEqual([{ ok: false, reason: 'one' }])
    expect(second.heard).toEqual([{ ok: false, reason: 'two' }])
  })

  it('is news (the notice) once no action is left in flight', () => {
    const h = playing()
    const { reply } = replies()
    h.session.manual(TAP, reply)
    h.deliver(h.taken(TAP))
    h.deliver(h.refuse('IllegalAnswer', 'late'))
    expect(h.state().notice).toMatch(/not allowed: late/)
  })
})

describe('a BadMessage and the combat preview in flight', () => {
  it('belongs to a manual action sent before the preview, which stays asked', () => {
    const h = playing()
    const { heard, reply } = replies()
    h.session.manual(TAP, reply)
    h.session.preview(12, BLOCK)
    h.deliver(h.refuse('BadMessage', 'the game is over'))
    expect(heard).toEqual([{ ok: false, reason: 'the game is over' }])
    // The preview is still waited for: its forecast lands, and the server is still taken to know previews.
    h.deliver({ type: 'Forecast', decision: 12, forecast: { attackers: [], creatures: [], players: [] } })
    expect(h.state().preview).toMatchObject({ decision: 12 })
    expect(h.session.preview(12, BLOCK)).toBe(true)
  })

  it('belongs to the preview when it was sent before the manual action', () => {
    const h = playing()
    const { heard, reply } = replies()
    h.session.preview(12, BLOCK)
    h.session.manual(TAP, reply)
    h.deliver(h.refuse('BadMessage', 'send each message as one JSON text frame of a known type'))
    expect(heard).toEqual([])
    expect(h.state().notice).toBeNull()
    expect(h.session.preview(12, BLOCK)).toBe(false)
  })

  it('with nobody listening to the manual action is its notice, not a swallowed preview refusal', () => {
    const h = playing()
    h.session.manual(TAP)
    h.session.preview(12, BLOCK)
    h.deliver(h.refuse('BadMessage', 'you are not in the game'))
    expect(h.state().notice).toMatch(/did not accept that: you are not in the game/)
    expect(h.session.preview(12, BLOCK)).toBe(true)
  })
})

describe('a manual action and a decision answer', () => {
  it('leaves an IllegalAnswer sent for an earlier answer to the notice', () => {
    const h = playing()
    const { heard, reply } = replies()
    expect(h.session.answer({ DeclareBlockers: [] })).toBe(true)
    h.session.manual(TAP, reply)
    h.deliver(h.refuse('IllegalAnswer', 'bad blocker'))
    expect(heard).toEqual([])
    expect(h.state().notice).toMatch(/not allowed: bad blocker/)
  })

  it('gives an IllegalAnswer to a manual action sent before the answer', () => {
    const h = playing()
    const { heard, reply } = replies()
    h.session.manual(TAP, reply)
    expect(h.session.answer({ DeclareBlockers: [] })).toBe(true)
    h.deliver(h.refuse('IllegalAnswer', 'already tapped'))
    expect(heard).toEqual([{ ok: false, reason: 'already tapped' }])
    expect(h.state().notice).toBeNull()
  })
})

describe('a connection that goes while an action is in flight', () => {
  it('tells the sender that the action may not have counted', () => {
    const h = playing()
    const { heard, reply } = replies()
    h.session.manual(TAP, reply)
    h.net.last.fail()
    expect(heard).toHaveLength(1)
    expect(heard[0]).toMatchObject({ ok: false })
    expect(heard[0] && 'reason' in heard[0] ? heard[0].reason : '').toMatch(/interrupted/)
  })

  it('is never sent, and never recorded, while the socket is not open', () => {
    const h = startSession()
    const reply = vi.fn()
    expect(h.session.manual(TAP, reply)).toBe(false)
    h.join()
    expect(reply).not.toHaveBeenCalled()
  })
})
