// Goal: the combat forecast in a table view is checked down to its leaves at the wire boundary: real forecasts and a hand-built one with a double block, an unblocked attacker and a lethal player pass unchanged, null is accepted, and a missing or mistyped field anywhere in it is a bad_shape error for the message that carried it, never an exception or a half-read forecast.
import { describe, expect, it } from 'vitest'
import { isRecord } from '@/net/check'
import type { CombatForecast } from '@/protocol/generated'
import { lateJoin, transcripts } from '@/test-utils/transcripts'
import { isCombatForecast, parseFrame } from './parse'

const wire = (value: unknown): string => JSON.stringify(value)

/** A copy of `node` with the value at `path` replaced; `undefined` removes the field on the wire. */
function put(node: unknown, path: readonly (string | number)[], value: unknown): unknown {
  const [head, ...rest] = path
  if (head === undefined) return value
  if (Array.isArray(node)) {
    const copy: unknown[] = [...node]
    copy[Number(head)] = put(node[Number(head)], rest, value)
    return copy
  }
  if (isRecord(node)) return { ...node, [head]: put(node[head], rest, value) }
  throw new Error(`no field ${String(head)} to replace`)
}

/**
 * Attacker 1 is blocked by 2 and 3 (a double block: 1 and 2 die), attacker 4 is blocked by 5 (nobody dies) and
 * attacker 6 is not blocked and takes seat 0 to a lethal total.
 */
const FORECAST: CombatForecast = {
  attackers: [
    {
      attacker: 1,
      target: { Player: 0 },
      blocked: true,
      blockers: [2, 3],
      deals: [
        { source: 1, target: { Object: 2 }, amount: 2 },
        { source: 1, target: { Object: 3 }, amount: 2 },
      ],
      receives: [
        { source: 2, target: { Object: 1 }, amount: 2 },
        { source: 3, target: { Object: 1 }, amount: 2 },
      ],
    },
    {
      attacker: 4,
      target: { Planeswalker: 9 },
      blocked: true,
      blockers: [5],
      deals: [{ source: 4, target: { Object: 5 }, amount: 1 }],
      receives: [{ source: 5, target: { Object: 4 }, amount: 1 }],
    },
    {
      attacker: 6,
      target: { Player: 0 },
      blocked: false,
      blockers: [],
      deals: [{ source: 6, target: { Player: 0 }, amount: 7 }],
      receives: [],
    },
  ],
  creatures: [
    { object: 1, damage: 4, dies: true },
    { object: 2, damage: 2, dies: true },
    { object: 3, damage: 2, dies: false },
    { object: 4, damage: 1, dies: false },
    { object: 5, damage: 1, dies: false },
    { object: 6, damage: 0, dies: false },
  ],
  players: [{ player: 0, damage: 7, life: 5, life_after: -2, lethal: true }],
}

const events = (): Record<string, unknown> => {
  const found = lateJoin.raw.find((item) => isRecord(item) && item.type === 'Events')
  if (!isRecord(found)) throw new Error('the fixture has no Events message')
  return found
}
const snapshot = (): Record<string, unknown> => {
  const found = lateJoin.raw.find((item) => isRecord(item) && item.type === 'Snapshot')
  if (!isRecord(found)) throw new Error('the fixture has no Snapshot message')
  return found
}

/** The same Events message carrying `forecast` (`undefined` removes the field). */
const eventsWith = (forecast: unknown): string => wire(put(events(), ['table', 'combat_forecast'], forecast))
/** The forecast below `path` inside a forecast edited at `path`. */
const edited = (path: readonly (string | number)[], value: unknown): string =>
  eventsWith(put(FORECAST, path, value))

describe('a combat forecast in a table view', () => {
  it('is accepted unchanged when the server sent one, in every recorded message', () => {
    const real = transcripts.flatMap(({ raw }) =>
      raw.flatMap((item) => {
        if (!isRecord(item)) return []
        const table = item.type === 'Events' ? item.table : isRecord(item.state) ? item.state : null
        return isRecord(table) && table.combat_forecast !== null ? [item] : []
      }),
    )
    expect(real.length).toBeGreaterThan(0)
    for (const item of real) expect(parseFrame(wire(item))).toMatchObject({ ok: true, message: item })
  })

  it.each([
    ['null', null],
    ['an empty forecast', { attackers: [], creatures: [], players: [] }],
    ['a double block, a block of a planeswalker attack and an unblocked attacker', FORECAST],
    ['a forecast with fields added later', { ...FORECAST, added_later: [1, 2] }],
  ])('%s travels in an Events message and in a Snapshot', (_what, forecast) => {
    const batch = parseFrame(eventsWith(forecast))
    expect(batch).toMatchObject({ ok: true })
    expect(batch.ok && batch.message.type === 'Events' && batch.message.table.combat_forecast).toEqual(
      forecast,
    )
    const state = put(snapshot(), ['state', 'combat_forecast'], forecast)
    expect(parseFrame(wire(state)).ok).toBe(true)
  })

  it('must be there: a table without the field is refused, and the error names the table', () => {
    for (const message of [
      eventsWith(undefined),
      wire(put(snapshot(), ['state', 'combat_forecast'], undefined)),
    ]) {
      const frame = parseFrame(message)
      expect(frame).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
      expect(!frame.ok && frame.error.detail).toMatch(/^(Events: "table"|Snapshot: "state") is not a /)
    }
  })

  const missing: [string, string[]][] = [
    ['attackers', ['attackers']],
    ['creatures', ['creatures']],
    ['players', ['players']],
    ['an attacker id', ['attackers', '0', 'attacker']],
    ['an attacker target', ['attackers', '0', 'target']],
    ['the blocked flag', ['attackers', '0', 'blocked']],
    ['the blockers', ['attackers', '0', 'blockers']],
    ['the damage an attacker deals', ['attackers', '0', 'deals']],
    ['the damage an attacker receives', ['attackers', '1', 'receives']],
    ['the source of a packet', ['attackers', '0', 'deals', '0', 'source']],
    ['the target of a packet', ['attackers', '0', 'deals', '0', 'target']],
    ['the amount of a packet', ['attackers', '0', 'receives', '1', 'amount']],
    ['a creature id', ['creatures', '0', 'object']],
    ['a creature damage', ['creatures', '1', 'damage']],
    ['the dies flag', ['creatures', '2', 'dies']],
    ['a player seat', ['players', '0', 'player']],
    ['a player damage', ['players', '0', 'damage']],
    ['a player life', ['players', '0', 'life']],
    ['a player life after', ['players', '0', 'life_after']],
    ['the lethal flag', ['players', '0', 'lethal']],
  ]

  it.each(missing)('without %s is a bad_shape error', (_what, path) => {
    const frame = parseFrame(edited(path, undefined))
    expect(frame).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
    expect(!frame.ok && frame.error.detail).toContain('Events: ')
  })

  const wrongTypes: [string, string[], unknown][] = [
    ['attackers as an object', ['attackers'], {}],
    ['attackers as text', ['attackers'], 'none'],
    ['an attacker that is a number', ['attackers', '0'], 3],
    ['an attacker id as text', ['attackers', '0', 'attacker'], 'one'],
    ['an attacker id as negative', ['attackers', '0', 'attacker'], -1],
    ['an attacker id as a fraction', ['attackers', '0', 'attacker'], 1.5],
    ['a target as text', ['attackers', '0', 'target'], 'Player'],
    ['a target with no key', ['attackers', '0', 'target'], {}],
    ['a target with two keys', ['attackers', '0', 'target'], { Player: 0, Planeswalker: 1 }],
    ['a target that is an unknown variant', ['attackers', '0', 'target'], { Battle: 3 }],
    ['a target from the damage side', ['attackers', '0', 'target'], { Object: 3 }],
    ['a target with a text seat', ['attackers', '0', 'target'], { Player: 'me' }],
    ['blocked as text', ['attackers', '0', 'blocked'], 'yes'],
    ['blocked as a number', ['attackers', '0', 'blocked'], 1],
    ['blockers as a number', ['attackers', '0', 'blockers'], 2],
    ['a blocker id as text', ['attackers', '0', 'blockers', '0'], '2'],
    ['deals as null', ['attackers', '0', 'deals'], null],
    ['a packet that is text', ['attackers', '0', 'deals', '0'], 'hit'],
    [
      'a packet target as a planeswalker variant',
      ['attackers', '0', 'deals', '0', 'target'],
      { Planeswalker: 2 },
    ],
    ['a packet target with a text id', ['attackers', '0', 'deals', '0', 'target'], { Object: 'x' }],
    ['a packet amount as negative', ['attackers', '0', 'deals', '0', 'amount'], -2],
    ['a packet amount as text', ['attackers', '0', 'receives', '0', 'amount'], '2'],
    ['a packet source as null', ['attackers', '2', 'deals', '0', 'source'], null],
    ['creatures as null', ['creatures'], null],
    ['a creature that is a list', ['creatures', '0'], [1, 2, true]],
    ['a creature id as text', ['creatures', '0', 'object'], 'one'],
    ['a creature damage as negative', ['creatures', '0', 'damage'], -1],
    ['a creature damage as a fraction', ['creatures', '0', 'damage'], 0.5],
    ['dies as text', ['creatures', '0', 'dies'], 'true'],
    ['dies as null', ['creatures', '0', 'dies'], null],
    ['players as an object', ['players'], { player: 0 }],
    ['a player that is null', ['players', '0'], null],
    ['a player seat as text', ['players', '0', 'player'], 'me'],
    ['a player damage as negative', ['players', '0', 'damage'], -7],
    ['a player life as a fraction', ['players', '0', 'life'], 5.5],
    ['a player life as text', ['players', '0', 'life'], '5'],
    ['life after as null', ['players', '0', 'life_after'], null],
    ['lethal as a number', ['players', '0', 'lethal'], 1],
  ]

  it.each(wrongTypes)('with %s is a bad_shape error', (_what, path, value) => {
    const frame = parseFrame(edited(path, value))
    expect(frame).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
  })

  it.each([
    ['a number', 3],
    ['text', 'forecast'],
    ['a list', []],
    ['an object with nothing in it', {}],
    ['false', false],
  ])('as %s instead of a forecast is a bad_shape error', (_what, value) => {
    expect(parseFrame(eventsWith(value))).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
  })

  it('keeps negative life: a player below zero is a forecast like any other', () => {
    expect(isCombatForecast(put(FORECAST, ['players', '0', 'life'], -4))).toBe(true)
    expect(isCombatForecast(put(FORECAST, ['players', '0', 'life_after'], -40))).toBe(true)
  })
})
