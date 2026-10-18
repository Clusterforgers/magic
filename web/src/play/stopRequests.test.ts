// Goal: the session sends one flat SetStop or FullControl message per request, changes the store only when the server says so, never sends a stop for a step the server refuses (untap, cleanup) or anything while the socket is closed, and asks for no yield while Full Control is on.
import { describe, expect, it } from 'vitest'
import type { FullControlMode, Step, StopSide } from '@/protocol/generated'
import { startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'
import { canHoldStop, STOP_STEPS } from './stops'

const SIDES: StopSide[] = ['Own', 'Opponent']
const MODES: FullControlMode[] = ['Off', 'NextWindow', 'Held']
const ALL_STEPS: Step[] = [
  'Untap',
  'Upkeep',
  'Draw',
  'Main1',
  'BeginCombat',
  'DeclareAttackers',
  'DeclareBlockers',
  'FirstStrikeDamage',
  'CombatDamage',
  'EndCombat',
  'Main2',
  'End',
  'Cleanup',
]
const REFUSED: Step[] = ['Untap', 'Cleanup']
const FLAGS = [true, false]

const cases = (steps: readonly Step[]): [StopSide, Step, boolean, boolean][] =>
  SIDES.flatMap((side) =>
    steps.flatMap((step) =>
      FLAGS.flatMap((on) => FLAGS.map((keep): [StopSide, Step, boolean, boolean] => [side, step, on, keep])),
    ),
  )

describe('which steps can hold a stop', () => {
  it('are the eleven where a player gets priority, in turn order', () => {
    expect([...STOP_STEPS]).toEqual(ALL_STEPS.filter((step) => !REFUSED.includes(step)))
  })

  it.each(ALL_STEPS)('says %s correctly', (step) => {
    expect(canHoldStop(step)).toBe(!REFUSED.includes(step))
  })
})

describe('setStop', () => {
  it.each(cases(STOP_STEPS))('sends %s %s on=%s keep=%s as one flat message', (side, step, on, keep) => {
    const h = startSession()
    h.join()
    expect(h.session.setStop(side, step, on, keep)).toBe(true)
    expect(h.sentOf('SetStop')).toEqual([{ type: 'SetStop', side, step, on, keep }])
    expect(h.sentTypes()).toEqual(['Join', 'SetStop'])
  })

  it.each(cases(REFUSED))(
    'sends nothing for %s %s on=%s keep=%s, which the server would refuse',
    (side, step, on, keep) => {
      const h = startSession()
      h.join()
      expect(h.session.setStop(side, step, on, keep)).toBe(false)
      expect(h.sentTypes()).toEqual(['Join'])
      expect(h.state().notice).toBeNull()
    },
  )

  it('leaves the stops as the server last said, until the server says otherwise', () => {
    const h = startSession()
    h.join()
    h.deliver({ type: 'Stops', stops: [{ side: 'Own', step: 'Main1', keep: true }] })
    const before = h.state().stops
    h.session.setStop('Opponent', 'End', true, false)
    h.session.setStop('Own', 'Main1', false, false)
    expect(h.state().stops).toBe(before)

    h.deliver({ type: 'Stops', stops: [{ side: 'Opponent', step: 'End', keep: false }] })
    expect(h.state().stops).toEqual([{ side: 'Opponent', step: 'End', keep: false }])
  })

  it('may be repeated: every call is a message, and the server answers a repeat with silence', () => {
    const h = startSession()
    h.join()
    h.session.setStop('Own', 'Draw', true, false)
    h.session.setStop('Own', 'Draw', true, false)
    h.session.setStop('Own', 'Draw', true, true)
    expect(h.sentOf('SetStop')).toEqual([
      { type: 'SetStop', side: 'Own', step: 'Draw', on: true, keep: false },
      { type: 'SetStop', side: 'Own', step: 'Draw', on: true, keep: false },
      { type: 'SetStop', side: 'Own', step: 'Draw', on: true, keep: true },
    ])
  })

  it('sends nothing while the socket is not open', () => {
    const h = startSession()
    expect(h.session.setStop('Own', 'Main1', true, false)).toBe(false)
    h.join()
    h.net.last.fail()
    expect(h.session.setStop('Own', 'Main1', true, false)).toBe(false)
    expect(h.sentTypes()).toEqual(['Join'])
  })

  it('is not part of the join: a seat that sets nothing sends nothing', () => {
    const h = startSession()
    h.join()
    expect(h.sentTypes()).toEqual(['Join'])
  })

  it('shows a refusal the server still makes as the usual notice, with no resync', () => {
    const h = startSession()
    h.join()
    h.session.setStop('Own', 'Main1', true, false)
    h.deliver({ type: 'Error', code: 'BadMessage', message: 'the game is over' })
    expect(h.state().notice).toMatch(/did not accept/)
    expect(h.sentTypes()).toEqual(['Join', 'SetStop'])
  })
})

describe('setFullControl', () => {
  it.each(MODES)('sends FullControl { mode: %s } as one flat message', (mode) => {
    const h = startSession()
    h.join()
    expect(h.session.setFullControl(mode)).toBe(true)
    expect(h.sentOf('FullControl')).toEqual([{ type: 'FullControl', mode }])
    expect(h.sentTypes()).toEqual(['Join', 'FullControl'])
  })

  it.each(MODES)('shows %s only once the server says so', (mode) => {
    const h = startSession()
    h.join()
    h.session.setFullControl(mode)
    expect(h.state().fullControl).toBe('Off')
    h.deliver({ type: 'FullControl', mode })
    expect(h.state().fullControl).toBe(mode)
  })

  it('follows the server when it turns the mode off by itself after the next window', () => {
    const h = startSession()
    h.join()
    h.session.setFullControl('NextWindow')
    h.deliver({ type: 'FullControl', mode: 'NextWindow' }, { type: 'FullControl', mode: 'Off' })
    expect(h.state().fullControl).toBe('Off')
  })

  it('sends nothing while the socket is not open', () => {
    const h = startSession()
    expect(h.session.setFullControl('Held')).toBe(false)
    h.join()
    h.net.last.fail()
    expect(h.session.setFullControl('Held')).toBe(false)
    expect(h.sentTypes()).toEqual(['Join'])
  })
})

describe('yields under Full Control', () => {
  it('are sent while Full Control is off', () => {
    const h = startSession()
    h.join()
    expect(h.session.yieldUntil('EndOfTurn')).toBe(true)
    expect(h.sentOf('Yield')).toEqual([{ type: 'Yield', until: 'EndOfTurn' }])
  })

  it.each(['NextWindow', 'Held'] satisfies FullControlMode[])(
    'are not sent while the server holds %s, which it refuses',
    (mode) => {
      const h = startSession()
      h.join()
      h.deliver({ type: 'FullControl', mode })
      expect(h.session.yieldUntil('EndOfTurn')).toBe(false)
      expect(h.session.yieldUntil('Resolve')).toBe(false)
      expect(h.session.yieldUntil('MyTurn')).toBe(false)
      expect(h.sentTypes()).toEqual(['Join'])
    },
  )

  it('are sent again once the server has turned Full Control off', () => {
    const h = startSession()
    h.join()
    h.deliver({ type: 'FullControl', mode: 'Held' })
    expect(h.session.yieldUntil('MyTurn')).toBe(false)
    h.deliver({ type: 'FullControl', mode: 'Off' })
    expect(h.session.yieldUntil('MyTurn')).toBe(true)
  })

  it('follow the mode a Welcome reports, including after a reconnect', () => {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    h.deliver({ ...welcome, full_control: 'Held' })
    expect(h.session.yieldUntil('EndOfTurn')).toBe(false)
    h.net.last.fail()
    h.timers.advance(250)
    h.open()
    h.deliver({ ...welcome, full_control: 'Off' })
    expect(h.session.yieldUntil('EndOfTurn')).toBe(true)
  })

  it('can still be cancelled while Full Control is on, since the server ends a yield when it starts', () => {
    const h = startSession()
    h.join()
    h.deliver({ type: 'Yielding', until: 'EndOfTurn' }, { type: 'FullControl', mode: 'Held' })
    expect(h.session.cancelYield()).toBe(true)
    expect(h.sentOf('CancelYield')).toEqual([{ type: 'CancelYield' }])
  })
})
