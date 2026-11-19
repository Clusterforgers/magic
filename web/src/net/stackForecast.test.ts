// Goal: the stack forecast is checked at the wire boundary down to its leaves: every change the engine sends passes, an object's forecast and a Forecast reply's `changes` may be left out, and a malformed line makes the message a bad_shape error, never a half-read forecast.
import { describe, expect, it } from 'vitest'
import { buildFixture } from '@/features/table/dev/fixture'
import { forecastStackTable } from '@/features/table/dev/previewScenes'
import { parseFrame } from './parse'
import { isForecastChange, isOptionalForecast } from './stackForecast'

const line = (change: unknown, subject: unknown = { Object: 3 }) => ({ subject, change })

describe('a forecast line', () => {
  it.each([
    ['damage', { Damage: { amount: 3 } }],
    ['stats', { Stats: { from: [4, 4], to: [7, -1] } }],
    ['counters', { Counters: { kind: '+1/+1', from: 0, to: 2 } }],
    ['named counters', { Counters: { kind: { Other: 'lore' }, from: 0, to: 1 } }],
    ['tapped', 'Tapped'],
    ['untapped', 'Untapped'],
    ['control', { Control: { to: 1 } }],
    ['a fate', { Leaves: { fate: 'Countered' } }],
    ['life', { Life: { from: 2, to: -3 } }],
  ])('passes %s', (_what, change) => {
    expect(isForecastChange(line(change))).toBe(true)
    expect(isForecastChange(line(change, { Player: 0 }))).toBe(true)
  })

  it.each([
    ['an unknown change', line({ Melts: {} })],
    ['an unknown fate', line({ Leaves: { fate: 'Melted' } })],
    ['negative damage', line({ Damage: { amount: -1 } })],
    ['stats that are no pair', line({ Stats: { from: [4], to: [7, 7] } })],
    ['a change with two keys', line({ Damage: { amount: 1 }, Life: { from: 1, to: 0 } })],
    ['a subject of another kind', line('Tapped', { Planeswalker: 3 })],
    ['a subject without an id', line('Tapped', { Object: 'x' })],
    ['no change', { subject: { Object: 3 } }],
  ])('refuses %s', (_what, value) => {
    expect(isForecastChange(value)).toBe(false)
  })

  it('may be left out of an object, but not be something else', () => {
    expect(isOptionalForecast(undefined)).toBe(true)
    expect(isOptionalForecast([])).toBe(true)
    expect(isOptionalForecast({})).toBe(false)
  })
})

describe('on the wire', () => {
  it('reads a snapshot whose top spell carries a forecast', () => {
    const { view } = forecastStackTable(buildFixture({ seats: 2 }))
    const frame = parseFrame(JSON.stringify({ type: 'Snapshot', seq: 1, state: view }))
    expect(frame.ok).toBe(true)
    const broken = {
      ...view,
      objects: view.objects.map((object) =>
        object.forecast === undefined ? object : { ...object, forecast: [1] },
      ),
    }
    expect(parseFrame(JSON.stringify({ type: 'Snapshot', seq: 1, state: broken })).ok).toBe(false)
  })

  it('keeps the lines of a target preview and refuses broken ones', () => {
    const changes = [line({ Damage: { amount: 3 } }), line({ Leaves: { fate: 'Dies' } })]
    const frame = parseFrame(JSON.stringify({ type: 'Forecast', decision: 4, forecast: null, changes }))
    expect(frame.ok && frame.message).toEqual({ type: 'Forecast', decision: 4, forecast: null, changes })
    const bad = parseFrame(JSON.stringify({ type: 'Forecast', decision: 4, forecast: null, changes: [{}] }))
    expect(bad.ok).toBe(false)
  })
})
