// Goal: the session holds the stops and the Full Control mode the server states (every Welcome and every Stops and FullControl message replace them whole), keeps them across a dropped connection until the next Welcome, sends nothing of its own, leaves the pending decision, the yield and the lobby alone, and ignores a notice it cannot read.
import { describe, expect, it } from 'vitest'
import type { FullControlMode, ServerMessage, Step, Stop, StopSide } from '@/protocol/generated'
import { priorityDecision, startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'

const MODES: FullControlMode[] = ['Off', 'NextWindow', 'Held']
const SIDES: StopSide[] = ['Own', 'Opponent']
const STEPS: Step[] = [
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
]

// A stop lasts (`keep`) or goes once its step has passed; most of these are one-turn stops, the default of a click.
const own = (step: Step, keep = false): Stop => ({ side: 'Own', step, keep })
const opponent = (step: Step, keep = false): Stop => ({ side: 'Opponent', step, keep })

const stopsMessage = (stops: Stop[]): ServerMessage => ({ type: 'Stops', stops })
const modeMessage = (mode: FullControlMode): ServerMessage => ({ type: 'FullControl', mode })

const LISTS: [string, Stop[]][] = [
  ['none', []],
  ['one on the seat’s own turn', [own('Main1')]],
  ['one lasting on an opponent’s turn', [opponent('End', true)]],
  [
    'both sides, lasting and one-turn',
    [own('Upkeep', true), own('End'), opponent('Draw', true), opponent('DeclareBlockers')],
  ],
  [
    'every step on both sides, alternating',
    SIDES.flatMap((side) => STEPS.map((step, n): Stop => ({ side, step, keep: n % 2 === 0 }))),
  ],
]

describe('what the server says', () => {
  it('starts with no stops and Full Control off', () => {
    const { stops, fullControl } = startSession().state()
    expect(stops).toEqual([])
    expect(fullControl).toBe('Off')
  })

  it.each(LISTS)('takes the stops of a Welcome (%s)', (_name, stops) => {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    h.deliver({ ...welcome, stops })
    expect(h.state().stops).toEqual(stops)
    expect(h.state().seat).toBe(0)
  })

  it.each(MODES)('takes the mode of a Welcome (%s)', (mode) => {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    h.deliver({ ...welcome, full_control: mode })
    expect(h.state().fullControl).toBe(mode)
  })

  it.each(LISTS)('replaces the whole list with each Stops message (%s)', (_name, stops) => {
    const h = startSession()
    h.join()
    h.deliver(stopsMessage([own('Draw'), opponent('Main2')]))
    expect(h.state().stops).toEqual([own('Draw'), opponent('Main2')])
    h.deliver(stopsMessage(stops))
    expect(h.state().stops).toEqual(stops)
  })

  it.each(MODES)('follows each FullControl message: %s, whatever the mode was', (mode) => {
    const h = startSession()
    h.join()
    for (const before of MODES) {
      h.deliver(modeMessage(before))
      expect(h.state().fullControl).toBe(before)
      h.deliver(modeMessage(mode))
      expect(h.state().fullControl).toBe(mode)
    }
  })

  it('changes the stops without the mode and the mode without the stops', () => {
    const h = startSession()
    h.join()
    h.deliver(stopsMessage([own('Main1')]), modeMessage('Held'))
    h.deliver(stopsMessage([]))
    expect(h.state()).toMatchObject({ stops: [], fullControl: 'Held' })
    h.deliver(stopsMessage([opponent('End')]), modeMessage('Off'))
    expect(h.state()).toMatchObject({ stops: [opponent('End')], fullControl: 'Off' })
  })

  it('keeps what it knew while the connection is down, and follows the Welcome of the next one', () => {
    const h = startSession()
    h.join()
    h.deliver(stopsMessage([own('Upkeep')]), modeMessage('Held'))

    // The connection drops: the server still holds them, so the page shows them until it is told otherwise.
    h.net.last.fail()
    expect(h.state()).toMatchObject({ stops: [own('Upkeep')], fullControl: 'Held' })

    // It comes back having forgotten them (a restart): the Welcome is the word that counts.
    h.timers.advance(250)
    h.open()
    h.deliver({ ...welcome, stops: [], full_control: 'Off' })
    expect(h.state()).toMatchObject({ stops: [], fullControl: 'Off' })

    // And when it remembers them, the Welcome says what it remembers.
    h.net.last.fail()
    h.timers.advance(250)
    h.open()
    h.deliver({ ...welcome, stops: [opponent('DeclareAttackers')], full_control: 'NextWindow' })
    expect(h.state()).toMatchObject({ stops: [opponent('DeclareAttackers')], fullControl: 'NextWindow' })
  })

  it('shows a lasting stop and a one-turn stop as the server told them, in the Welcome and in a notice', () => {
    const lasting = own('Main1', true)
    const oneTurn = opponent('End')
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    h.deliver({ ...welcome, stops: [lasting, oneTurn] })
    expect(h.state().stops).toStrictEqual([lasting, oneTurn])
    expect(h.state().stops.map((stop) => stop.keep)).toEqual([true, false])

    h.deliver(stopsMessage([oneTurn, opponent('Upkeep', true)]))
    expect(h.state().stops.map((stop) => [stop.step, stop.keep])).toEqual([
      ['End', false],
      ['Upkeep', true],
    ])
  })

  it('follows a stop upgraded to lasting and downgraded again by the list the server tells each time', () => {
    const h = startSession()
    h.join()
    const other = opponent('Draw', true)
    // The same stop in the same place of the list, with the other keep each time.
    for (const keep of [false, true, false, true, true, false]) {
      h.deliver(stopsMessage([own('Main1', keep), other]))
      expect(h.state().stops).toEqual([own('Main1', keep), other])
    }
  })

  it('hands on the stops of a Welcome and of a notice as the server listed them', () => {
    const h = startSession()
    h.join()
    const listed = [own('Upkeep', true), own('End'), opponent('Draw', true)]
    h.deliver(stopsMessage(listed))
    expect(h.state().stops).toStrictEqual(listed)
  })
})

describe('what the session leaves alone', () => {
  it('sends nothing of its own in answer to any of it', () => {
    const h = startSession()
    h.join()
    h.deliver(
      stopsMessage([own('Main1'), opponent('End')]),
      modeMessage('Held'),
      stopsMessage([]),
      modeMessage('Off'),
    )
    h.deliver({ ...welcome, stops: [own('Draw')], full_control: 'NextWindow' })
    expect(h.sentTypes()).toEqual(['Join'])
  })

  it('leaves the pending decision, its clock and the yield alone', () => {
    const h = startSession()
    h.join()
    h.deliver({ type: 'Yielding', until: 'MyTurn' })
    h.deliver({ ...priorityDecision(7), deadline_ms: 10_000 })
    // A yielding seat is not sent priority, so give it another kind of pending decision to compare.
    const before = h.state()
    h.deliver(stopsMessage([own('Main1')]), modeMessage('Held'))
    const after = h.state()
    expect(after.pending).toBe(before.pending)
    expect(after.clock).toBe(before.clock)
    expect(after.yielding).toBe('MyTurn')
    expect(after.lobby).toBe(before.lobby)
    expect(after.notice).toBeNull()
  })

  it('does not make a stop of anything in the game: the view, the log and the phase stay', () => {
    const h = startSession()
    h.join()
    const before = h.state()
    h.deliver(stopsMessage([own('Main1')]), modeMessage('Held'))
    const after = h.state()
    expect(after.view).toBe(before.view)
    expect(after.log).toBe(before.log)
    expect(after.phase).toBe(before.phase)
    expect(after.seq).toBe(before.seq)
  })
})

describe('the end of the game and notices it cannot read', () => {
  it('shows what the server last said when the game ends: it empties them first', () => {
    const h = startSession()
    h.join()
    h.deliver(stopsMessage([own('Main1')]), modeMessage('Held'))
    h.deliver(stopsMessage([]), modeMessage('Off'), { type: 'GameOver', result: 'Draw' })
    expect(h.state()).toMatchObject({ stops: [], fullControl: 'Off', phase: 'over' })
  })

  it.each([
    ['nothing', { type: 'Stops' }],
    ['null', { type: 'Stops', stops: null }],
    ['one stop instead of a list', { type: 'Stops', stops: { side: 'Own', step: 'Main1', keep: true } }],
    ['a stop that is text', { type: 'Stops', stops: ['Main1'] }],
    ['an unknown side', { type: 'Stops', stops: [{ side: 'Both', step: 'Main1', keep: true }] }],
    ['an unknown step', { type: 'Stops', stops: [{ side: 'Own', step: 'Combat', keep: true }] }],
    ['a step in lower case', { type: 'Stops', stops: [{ side: 'Own', step: 'main1', keep: true }] }],
    ['a stop without keep', { type: 'Stops', stops: [{ side: 'Own', step: 'Main1' }] }],
    ['keep as text', { type: 'Stops', stops: [{ side: 'Own', step: 'Main1', keep: 'yes' }] }],
    ['keep as a number', { type: 'Stops', stops: [{ side: 'Own', step: 'Main1', keep: 1 }] }],
    ['keep as null', { type: 'Stops', stops: [{ side: 'Own', step: 'Main1', keep: null }] }],
    [
      'a stop twice',
      {
        type: 'Stops',
        stops: [
          { side: 'Own', step: 'Main1', keep: true },
          { side: 'Own', step: 'Main1', keep: true },
        ],
      },
    ],
    [
      'a stop twice with different keeps',
      {
        type: 'Stops',
        stops: [
          { side: 'Own', step: 'Main1', keep: true },
          { side: 'Own', step: 'Main1', keep: false },
        ],
      },
    ],
  ])('ignores a Stops notice with %s and says so', (_what, frame) => {
    const h = startSession()
    h.join()
    h.deliver(stopsMessage([opponent('End', true)]), frame)
    expect(h.state().stops).toEqual([opponent('End', true)])
    expect(h.state().notice).toMatch(/could not read/)
  })

  it.each([
    ['nothing', { type: 'FullControl' }],
    ['an unknown word', { type: 'FullControl', mode: 'Always' }],
    ['the word in lower case', { type: 'FullControl', mode: 'held' }],
    ['a flag', { type: 'FullControl', mode: true }],
    ['null', { type: 'FullControl', mode: null }],
  ])('ignores a FullControl notice with %s and says so', (_what, frame) => {
    const h = startSession()
    h.join()
    h.deliver(modeMessage('NextWindow'), frame)
    expect(h.state().fullControl).toBe('NextWindow')
    expect(h.state().notice).toMatch(/could not read/)
  })

  it.each([
    ['no stops', { ...welcome, stops: undefined }],
    ['stops that are not a list', { ...welcome, stops: 'Main1' }],
    ['a stop on an unknown step', { ...welcome, stops: [{ side: 'Own', step: 'Combat', keep: true }] }],
    ['a stop without keep', { ...welcome, stops: [{ side: 'Own', step: 'Main1' }] }],
    ['no mode', { ...welcome, full_control: undefined }],
    ['an unknown mode', { ...welcome, full_control: 'Always' }],
  ])('ignores a Welcome with %s, and so never takes the seat', (_what, frame) => {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    h.deliver(frame)
    expect(h.state().seat).toBeNull()
    expect(h.state().stops).toEqual([])
    expect(h.state().fullControl).toBe('Off')
  })
})
