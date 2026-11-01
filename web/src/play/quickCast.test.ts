// Goal: a click on an instant while the server passes for the seat asks for its next window (only when no Full Control mode already asks), casts the card at that window when it is castable there and leaves the decision on screen when not, and cancelling gives back only the window it asked for.
import { describe, expect, it } from 'vitest'
import type { FullControlMode } from '@/protocol/generated'
import { startSession } from '@/test-utils/harness'

const CARD = 7

function priority(id: number, castable: number[]) {
  const options = { playable_lands: [], castable, mana_abilities: [], can_act: castable.length > 0, can_pass: true }
  return { type: 'Decision', id, decision: { player: 0, kind: { Priority: options } } }
}

describe('castWhenAsked', () => {
  it.each([
    ['Off', [{ type: 'FullControl', mode: 'NextWindow' }]],
    ['NextWindow', []],
    ['Held', []],
  ] as [FullControlMode, unknown[]][])('in mode %s asks for the next window with %j', (mode, expected) => {
    const h = startSession()
    h.join()
    h.deliver({ type: 'FullControl', mode })
    expect(h.session.castWhenAsked(CARD)).toBe(true)
    expect(h.sentOf('FullControl')).toEqual(expected)
    expect(h.state().castWhenAsked).toBe(CARD)
  })

  it.each([
    ['castable', [CARD], [{ Priority: { CastSpell: CARD } }], null],
    ['not castable', [], [], 3],
    ['another card castable', [CARD + 1], [], 3],
  ] as const)('at the window, a card that is %s', (_, castable, answers, pending) => {
    const h = startSession()
    h.join()
    h.session.castWhenAsked(CARD)
    h.deliver(priority(3, [...castable]))
    const sent = h.sentOf('Answer') as { answer: unknown }[]
    expect(sent.map((message) => message.answer)).toEqual(answers)
    expect(h.state().pending?.id ?? null).toBe(pending)
    expect(h.state().castWhenAsked).toBeNull()
  })

  it('waits through a decision that is not priority', () => {
    const h = startSession()
    h.join()
    h.session.castWhenAsked(CARD)
    h.deliver({
      type: 'Decision',
      id: 4,
      decision: { player: 0, kind: { DeclareBlockers: { blockers: [], attackers: [] } } },
    })
    expect(h.sentOf('Answer')).toEqual([])
    expect(h.state().castWhenAsked).toBe(CARD)
  })

  it.each([
    ['NextWindow', [{ type: 'FullControl', mode: 'NextWindow' }, { type: 'FullControl', mode: 'Off' }]],
    ['Held', [{ type: 'FullControl', mode: 'NextWindow' }]],
  ] as [FullControlMode, unknown[]][])('cancelling in mode %s sends %j', (mode, expected) => {
    const h = startSession()
    h.join()
    h.session.castWhenAsked(CARD)
    h.deliver({ type: 'FullControl', mode })
    expect(h.session.castWhenAsked(null)).toBe(true)
    expect(h.sentOf('FullControl')).toEqual(expected)
    expect(h.state().castWhenAsked).toBeNull()
  })

  it('sends nothing while the socket is closed', () => {
    const h = startSession()
    expect(h.session.castWhenAsked(CARD)).toBe(false)
    expect(h.state().castWhenAsked).toBeNull()
  })
})
