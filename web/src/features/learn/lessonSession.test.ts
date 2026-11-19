// Goal: a lesson's session sends an answer only when the current step allows it, holds every other input that changes the game (manual actions, Concede, Full Control, stops, yields, automatic choices, take-back) and says why, lets the step's own stop or End turn through, passes preview, chat and resync, hands the table a store whose pending decision is narrowed, and lets everything through once the coach is gone.
import { describe, expect, it, vi } from 'vitest'
import type { Session } from '@/play/session'
import { createPlayStore } from '@/play/store'
import type { ObjectView } from '@/protocol/generated'
import { lessonSession } from './lessonSession'
import type { Gate } from './lessonSession'

const HELD = [
  'manual',
  'concede',
  'setFullControl',
  'setEndTurnByHand',
  'setAutoChoices',
  'autoYield',
  'clearAutoYield',
  'autoSkip',
  'cancelAutoSkip',
  'castWhenAsked',
  'takeBack',
  'voteTakeBack',
] as const
const PASSED = ['chat', 'resync', 'preview', 'cancelYield'] as const

function fakeSession(): Session {
  const store = createPlayStore()
  store.setState({
    objectsById: {
      1: { id: 1, name: 'Forest' } as ObjectView,
      2: { id: 2, name: 'Grizzly Bears' } as ObjectView,
    },
  })
  const calls = Object.fromEntries(
    [...HELD, ...PASSED, 'answer', 'setStop', 'yieldUntil'].map((name) => [name, vi.fn(() => true)]),
  )
  return { store, ...calls } as unknown as Session
}

const LAND: Gate = { step: 'land', allow: { kind: 'land', name: 'Forest' }, hint: undefined }

describe('lessonSession', () => {
  it('sends the allowed answer and holds another, with the step’s reason', () => {
    const inner = fakeSession()
    const session = lessonSession(inner, () => 42)
    session.setGate(LAND)
    expect(session.answer({ Priority: { CastSpell: 2 } })).toBe(false)
    expect(inner.answer).not.toHaveBeenCalled()
    expect(session.coach.getState().refused).toEqual({ reason: 'Not yet: play the Forest first.', at: 42 })
    expect(session.answer({ Priority: { PlayLand: 1 } })).toBe(true)
    expect(inner.answer).toHaveBeenCalledWith({ Priority: { PlayLand: 1 } })
  })

  it('says a step’s own hint for a held input', () => {
    const session = lessonSession(fakeSession())
    session.setGate({ ...LAND, hint: 'Choose the Forest.' })
    session.concede()
    expect(session.coach.getState().refused?.reason).toBe('Choose the Forest.')
  })

  it.each(HELD)('holds %s by default', (name) => {
    const inner = fakeSession()
    const session = lessonSession(inner)
    session.setGate(LAND)
    const method = session[name] as (...args: unknown[]) => boolean
    expect(method(1, true)).toBe(false)
    expect(inner[name]).not.toHaveBeenCalled()
  })

  it.each(PASSED)('passes %s', (name) => {
    const inner = fakeSession()
    const session = lessonSession(inner)
    session.setGate(LAND)
    const method = session[name] as (...args: unknown[]) => boolean
    method('x')
    expect(inner[name]).toHaveBeenCalled()
  })

  it('lets the step’s stop through and holds another', () => {
    const inner = fakeSession()
    const session = lessonSession(inner)
    session.setGate({ step: 'stop', allow: { kind: 'stop', side: 'Opponent', step: 'End' }, hint: undefined })
    expect(session.setStop('Own', 'End', true, true)).toBe(false)
    expect(session.setStop('Opponent', 'End', false, true)).toBe(false)
    expect(session.setStop('Opponent', 'End', true, true)).toBe(true)
    expect(inner.setStop).toHaveBeenCalledOnce()
  })

  it('lets End turn through only at its step', () => {
    const inner = fakeSession()
    const session = lessonSession(inner)
    session.setGate(LAND)
    expect(session.yieldUntil('EndOfTurn')).toBe(false)
    session.setGate({ step: 'end-turn', allow: { kind: 'endTurn' }, hint: undefined })
    expect(session.yieldUntil('MyTurn')).toBe(false)
    expect(session.yieldUntil('EndOfTurn')).toBe(true)
  })

  it('narrows the pending decision of the store the table reads, and follows the gate', () => {
    const inner = fakeSession()
    const session = lessonSession(inner)
    inner.store.setState({
      pending: {
        id: 1,
        decision: {
          player: 0,
          kind: { Priority: { playable_lands: [1], castable: [2], mana_abilities: [] } },
        },
      } as never,
    })
    const castable = (): unknown => {
      const kind = session.store.getState().pending?.decision.kind
      return kind !== undefined && 'Priority' in kind ? kind.Priority.castable : null
    }
    expect(castable()).toEqual([2])
    session.setGate(LAND)
    expect(castable()).toEqual([])
    session.setGate({ step: 'cast', allow: { kind: 'cast', name: 'Grizzly Bears' }, hint: undefined })
    expect(castable()).toEqual([2])
  })

  it('lets everything through once the coach is gone', () => {
    const inner = fakeSession()
    const session = lessonSession(inner)
    session.setGate(LAND)
    session.setGate(null)
    expect(session.concede()).toBe(true)
    expect(session.answer({ Priority: 'Pass' })).toBe(true)
  })
})
