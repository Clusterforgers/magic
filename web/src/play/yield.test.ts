// Goal: the session asks for a yield and for its end with one flat message each and changes nothing itself; what the table shows follows the server's word (every Welcome and every Yielding), a yield that starts takes the priority decision on screen with it and leaves every other decision, one that ends leaves the pending decision alone, the game ending clears it, and a notice the page cannot read changes nothing.
import { describe, expect, it } from 'vitest'
import type { ServerMessage, YieldUntil } from '@/protocol/generated'
import { decisionFor } from '@/test-utils/decisions'
import { priorityDecision, startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'

const UNTIL: YieldUntil[] = ['Resolve', 'EndOfTurn', 'MyTurn']

const yielding = (until: YieldUntil | null): ServerMessage => ({ type: 'Yielding', until })

const attackDecision = (id: number): ServerMessage => ({
  type: 'Decision',
  id,
  decision: decisionFor('DeclareAttackers'),
})

describe('asking', () => {
  it.each(UNTIL)('sends Yield { until: %s } as one flat message and changes nothing by itself', (until) => {
    const h = startSession()
    h.join()
    expect(h.session.yieldUntil(until)).toBe(true)
    expect(h.sentOf('Yield')).toEqual([{ type: 'Yield', until }])
    expect(h.state().yielding).toBeNull()
  })

  it('does not take the priority decision away before the server says it passed', () => {
    const h = startSession()
    h.join()
    h.deliver(priorityDecision(7))
    h.session.yieldUntil('EndOfTurn')
    expect(h.state().pending).toMatchObject({ id: 7 })
  })

  it('sends CancelYield as one message and leaves the state to the server', () => {
    const h = startSession()
    h.join()
    h.deliver(yielding('MyTurn'))
    expect(h.session.cancelYield()).toBe(true)
    expect(h.sentOf('CancelYield')).toEqual([{ type: 'CancelYield' }])
    expect(h.state().yielding).toBe('MyTurn')
  })

  it('sends nothing while the socket is not open', () => {
    const h = startSession()
    expect(h.session.yieldUntil('Resolve')).toBe(false)
    expect(h.session.cancelYield()).toBe(false)
    h.join()
    h.net.last.fail()
    expect(h.session.yieldUntil('Resolve')).toBe(false)
    expect(h.session.cancelYield()).toBe(false)
    expect(h.sentOf('Yield')).toEqual([])
    expect(h.sentOf('CancelYield')).toEqual([])
  })

  it('does not hold requests back: every call is a message', () => {
    const h = startSession()
    h.join()
    h.session.yieldUntil('Resolve')
    h.session.yieldUntil('Resolve')
    h.session.cancelYield()
    expect(h.sentTypes()).toEqual(['Join', 'Yield', 'Yield', 'CancelYield'])
  })
})

describe('what the server says', () => {
  it('starts without a yield', () => {
    expect(startSession().state().yielding).toBeNull()
  })

  it.each([...UNTIL, null])('takes the yield of a Welcome (%s)', (until) => {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    h.deliver({ ...welcome, yielding: until })
    expect(h.state().yielding).toBe(until)
  })

  it('follows the Welcome of a later connection, whatever the page held before', () => {
    const h = startSession()
    h.join()
    h.deliver(yielding('EndOfTurn'))
    expect(h.state().yielding).toBe('EndOfTurn')

    // The connection drops and the server comes back having forgotten it (a restart): its word is none.
    h.net.last.fail()
    expect(h.state().yielding).toBe('EndOfTurn')
    h.timers.advance(250)
    h.open()
    h.deliver({ ...welcome, yielding: null })
    expect(h.state().yielding).toBeNull()
  })

  it.each(UNTIL)('shows each change of the yield: %s on, then replaced, then off', (until) => {
    const h = startSession()
    h.join()
    h.deliver(yielding(until))
    expect(h.state().yielding).toBe(until)
    h.deliver(yielding('MyTurn'))
    expect(h.state().yielding).toBe('MyTurn')
    h.deliver(yielding(null))
    expect(h.state().yielding).toBeNull()
  })
})

describe('the decision on screen', () => {
  it('goes with a yield that starts, clock and all, since the server passed for it', () => {
    const h = startSession()
    h.join()
    const baseline = h.timers.pending().length
    h.deliver({ ...priorityDecision(7), deadline_ms: 10_000 })
    expect(h.state().clock).not.toBeNull()
    expect(h.timers.pending()).toHaveLength(baseline + 1)
    h.deliver(yielding('EndOfTurn'))
    expect(h.state().pending).toBeNull()
    expect(h.state().clock).toBeNull()
    expect(h.timers.pending(), 'the countdown stopped with it').toHaveLength(baseline)
  })

  it.each([...UNTIL])('goes with a yield of %s, whichever yield it replaces', (until) => {
    const h = startSession()
    h.join()
    h.deliver(yielding('MyTurn'), priorityDecision(9))
    expect(h.state().pending).toMatchObject({ id: 9 })
    h.deliver(yielding(until))
    expect(h.state().pending).toBeNull()
  })

  it('stays when the decision is another kind: the seat still declares attackers, blocks and discards', () => {
    const h = startSession()
    h.join()
    h.deliver(attackDecision(5), yielding('EndOfTurn'))
    expect(h.state().pending).toMatchObject({ id: 5 })
    expect(h.state().yielding).toBe('EndOfTurn')
  })

  it('stays when the yield ends, since the decision that ended it follows the notice', () => {
    const h = startSession()
    h.join()
    h.deliver(yielding('EndOfTurn'), yielding(null), priorityDecision(10))
    expect(h.state().pending).toMatchObject({ id: 10 })
    expect(h.state().yielding).toBeNull()
  })

  it('is not taken by a yield that is already over when a repeated notice arrives', () => {
    const h = startSession()
    h.join()
    h.deliver(priorityDecision(4), yielding(null))
    expect(h.state().pending).toMatchObject({ id: 4 })
  })
})

describe('the end of the game and notices it cannot read', () => {
  it('clears the yield when the game is over, with no decision left either', () => {
    const h = startSession()
    h.join()
    h.deliver(yielding('MyTurn'), { type: 'GameOver', result: 'Draw' })
    expect(h.state()).toMatchObject({ yielding: null, pending: null, phase: 'over' })
  })

  it.each([
    ['an unknown word', { type: 'Yielding', until: 'Soon' }],
    ['a number', { type: 'Yielding', until: 3 }],
    ['nothing', { type: 'Yielding' }],
    ['the word in lower case', { type: 'Yielding', until: 'endofturn' }],
  ])('ignores a notice with %s and says so', (_what, frame) => {
    const h = startSession()
    h.join()
    h.deliver(yielding('Resolve'), frame)
    expect(h.state().yielding).toBe('Resolve')
    expect(h.state().notice).toMatch(/could not read/)
  })

  it('ignores a Welcome that cannot say what the server holds, and so never takes the seat', () => {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    h.deliver({ ...welcome, yielding: 'Soon' })
    expect(h.state().seat).toBeNull()
    expect(h.state().yielding).toBeNull()
  })
})
