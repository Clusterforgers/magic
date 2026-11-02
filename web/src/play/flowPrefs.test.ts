// Goal: the saved pace preferences (default stops, end turn by hand, auto choices) survive in storage, keep only pinned stops, and reach the server at Welcome: the default stops once per game and only to a seat with none, the end-turn choice whenever the server holds the other one.
import { beforeEach, describe, expect, it } from 'vitest'
import type { Stop, StopSide } from '@/protocol/generated'
import { startSession } from '@/test-utils/harness'
import {
  APPLIED_LIMIT,
  DEFAULT_STOPS_KEY,
  flowPrefs,
  resetDefaultStops,
  resetFlowPrefs,
  saveDefaultStops,
  saveEndTurnByHand,
  welcomeRequests,
} from './flowPrefs'
import { STOP_STEPS } from './stops'

const SIDES: StopSide[] = ['Own', 'Opponent']
const pinned = (side: StopSide, step: Stop['step']): Stop => ({ side, step, keep: true })
const OFF = { order_triggers: false, assign_damage: false, smart_stops: false }
const BOTH = { order_triggers: true, assign_damage: true, smart_stops: true }

beforeEach(() => {
  window.localStorage.clear()
  resetFlowPrefs()
})

describe('default stops', () => {
  it.each(SIDES.flatMap((side) => STOP_STEPS.map((step) => [side, step] as const)))(
    'keeps %s %s pinned across a reload, and drops one-turn stops',
    (side, step) => {
      saveDefaultStops([pinned(side, step), { side, step: 'Upkeep', keep: false }])
      resetFlowPrefs()
      expect(flowPrefs().defaultStops).toEqual([pinned(side, step)])
    },
  )

  it('starts with none, resets to none, and ignores stored junk', () => {
    expect(flowPrefs().defaultStops).toEqual([])
    saveDefaultStops([pinned('Own', 'End')])
    resetDefaultStops()
    resetFlowPrefs()
    expect(flowPrefs().defaultStops).toEqual([])
    window.localStorage.setItem(
      DEFAULT_STOPS_KEY,
      JSON.stringify([
        { side: 'Own', step: 'Cleanup' },
        { side: 'Nobody', step: 'End' },
        7,
        pinned('Opponent', 'Draw'),
      ]),
    )
    resetFlowPrefs()
    expect(flowPrefs().defaultStops).toEqual([pinned('Opponent', 'Draw')])
  })
})

describe('welcome requests', () => {
  const stops = [pinned('Own', 'Main1'), pinned('Opponent', 'End')]
  const prefs = { defaultStops: stops, endTurnByHand: false, autoChoices: OFF }
  const setStops = stops.map(({ side, step }) => ({ type: 'SetStop', side, step, on: true, keep: true }))

  it.each([
    ['a new game and a seat with no stops', [], setStops],
    ['a seat that already holds stops', [pinned('Own', 'Draw')], []],
  ] as const)('gives the default stops to %s', (_, held, expected) => {
    expect(
      welcomeRequests({ game: 'g1', stops: held, endTurnByHand: false, autoChoices: OFF }, prefs),
    ).toEqual(expected)
  })

  it('gives them once per game, also after a reconnect to a seat that cleared them', () => {
    expect(welcomeRequests({ game: 'g1', stops: [], endTurnByHand: false, autoChoices: OFF }, prefs)).toEqual(
      setStops,
    )
    expect(welcomeRequests({ game: 'g1', stops: [], endTurnByHand: false, autoChoices: OFF }, prefs)).toEqual(
      [],
    )
    expect(welcomeRequests({ game: 'g2', stops: [], endTurnByHand: false, autoChoices: OFF }, prefs)).toEqual(
      setStops,
    )
  })

  it('remembers only the latest games', () => {
    for (let n = 0; n <= APPLIED_LIMIT; n += 1)
      welcomeRequests({ game: `g${n}`, stops: [], endTurnByHand: false, autoChoices: OFF }, prefs)
    expect(welcomeRequests({ game: 'g0', stops: [], endTurnByHand: false, autoChoices: OFF }, prefs)).toEqual(
      setStops,
    )
  })

  it.each([
    [true, false, [{ type: 'EndTurnByHand', on: true }]],
    [false, true, [{ type: 'EndTurnByHand', on: false }]],
    [true, true, []],
    [false, false, []],
  ])('with end turn by hand saved %s and held %s sends %j', (saved, held, expected) => {
    const none = { defaultStops: [], endTurnByHand: saved, autoChoices: OFF }
    expect(welcomeRequests({ game: 'g1', stops: [], endTurnByHand: held, autoChoices: OFF }, none)).toEqual(
      expected,
    )
  })

  it.each([
    [OFF, BOTH, [{ type: 'AutoChoices', choices: OFF }]],
    [BOTH, OFF, [{ type: 'AutoChoices', choices: BOTH }]],
    [BOTH, BOTH, []],
    [OFF, OFF, []],
    [
      { ...BOTH, assign_damage: false },
      BOTH,
      [{ type: 'AutoChoices', choices: { ...BOTH, assign_damage: false } }],
    ],
    [{ ...OFF, smart_stops: true }, OFF, [{ type: 'AutoChoices', choices: { ...OFF, smart_stops: true } }]],
  ])('with auto choices saved %j and held %j sends %j', (saved, held, expected) => {
    const prefs = { defaultStops: [], endTurnByHand: false, autoChoices: saved }
    expect(
      welcomeRequests({ game: 'g1', stops: [], endTurnByHand: false, autoChoices: held }, prefs),
    ).toEqual(expected)
  })
})

describe('in a session', () => {
  it('sends the saved preferences after the Welcome and stores the server word', () => {
    saveDefaultStops([pinned('Opponent', 'End')])
    saveEndTurnByHand(true)
    const h = startSession()
    h.join()
    expect(h.sentTypes()).toEqual(['Join', 'SetStop', 'EndTurnByHand'])
    expect(h.state().endTurnByHand).toBe(false)
    h.deliver({ type: 'EndTurnByHand', on: true })
    expect(h.state().endTurnByHand).toBe(true)
  })

  it('setAutoChoices sends the choices and saves them for later games', () => {
    const h = startSession()
    h.join()
    expect(h.session.setAutoChoices(BOTH)).toBe(true)
    expect(h.sentOf('AutoChoices')).toEqual([{ type: 'AutoChoices', choices: BOTH }])
    resetFlowPrefs()
    expect(flowPrefs().autoChoices).toEqual(BOTH)
  })

  it('setEndTurnByHand sends the choice and saves it for later games', () => {
    const h = startSession()
    h.join()
    expect(h.session.setEndTurnByHand(true)).toBe(true)
    expect(h.sentOf('EndTurnByHand')).toEqual([{ type: 'EndTurnByHand', on: true }])
    resetFlowPrefs()
    expect(flowPrefs().endTurnByHand).toBe(true)
  })
})
