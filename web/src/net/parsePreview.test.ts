// Goal: a Forecast message, the engine's reply to a Preview, is checked at the wire boundary: the decision it answers must be a whole number and the forecast null or a combat forecast checked down to its leaves; every reply recorded from the engine passes unchanged, and a missing, mistyped or malformed part is a bad_shape error for that message, never an exception or a half-read forecast.
import { describe, expect, it } from 'vitest'
import { attacksScene, blocksScene } from '@/test-utils/previewFixture'
import { parseFrame } from './parse'

const wire = (value: unknown): string => JSON.stringify(value)

/** The first forecast the engine gave in the recorded blocks. */
const [FORECAST] = blocksScene.exchanges.flatMap(({ reply }) =>
  reply.forecast === null ? [] : [reply.forecast],
)
if (FORECAST === undefined) throw new Error('the fixture holds no forecast')

/** The reply with `change` applied to its fields (`undefined` removes one from the wire). */
const reply = (change: Record<string, unknown>): string =>
  wire({ type: 'Forecast', decision: 12, forecast: FORECAST, ...change })

describe('a Forecast message', () => {
  const recorded = [...blocksScene.exchanges, ...attacksScene.exchanges]

  it.each(recorded.map((exchange) => [exchange.label, exchange.rawReply] as const))(
    'recorded from the engine (%s) is accepted unchanged',
    (_label, raw) => {
      expect(parseFrame(wire(raw))).toMatchObject({ ok: true, message: raw })
    },
  )

  it.each([
    ['no forecast', null],
    ['an empty forecast', { attackers: [], creatures: [], players: [] }],
    ['a forecast with fields added later', { ...FORECAST, added_later: [1] }],
  ])('carries %s', (_what, forecast) => {
    const frame = parseFrame(reply({ forecast }))
    expect(frame.ok && frame.message).toEqual({ type: 'Forecast', decision: 12, forecast })
  })

  it.each([
    ['no decision', { decision: undefined }],
    ['a decision that is text', { decision: '12' }],
    ['a decision that is negative', { decision: -1 }],
    ['a decision that is a fraction', { decision: 1.5 }],
    ['a decision that is null', { decision: null }],
    ['no forecast field at all', { forecast: undefined }],
    ['a forecast that is text', { forecast: 'none' }],
    ['a forecast that is a number', { forecast: 0 }],
    ['a forecast that is a list', { forecast: [] }],
    ['a forecast that is an empty object', { forecast: {} }],
    ['a forecast without players', { forecast: { attackers: [], creatures: [] } }],
    ['a forecast whose attackers are an object', { forecast: { ...FORECAST, attackers: {} } }],
  ])('without a usable part, %s, is a bad_shape error for the message', (_what, change) => {
    const frame = parseFrame(reply(change))
    expect(frame).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
    expect(!frame.ok && frame.error.detail).toMatch(/^Forecast: "(decision|forecast)"/)
  })

  it.each([
    ['a creature that does not say whether it dies', ['creatures', 0, 'dies'], 'yes'],
    ['a creature with a negative damage', ['creatures', 0, 'damage'], -1],
    ['a player whose life is text', ['players', 0, 'life'], '6'],
    ['a player whose lethal flag is missing', ['players', 0, 'lethal'], undefined],
    ['an attacker whose blockers are a number', ['attackers', 0, 'blockers'], 2],
    ['an attacker with an unknown target', ['attackers', 0, 'target'], { Battle: 1 }],
    ['a packet with a text amount', ['attackers', 0, 'deals', 0, 'amount'], '2'],
  ])('with a broken leaf, %s, is refused whole', (_what, path, value) => {
    const forecast: unknown = JSON.parse(wire(FORECAST))
    let node: unknown = forecast
    for (const step of path.slice(0, -1)) {
      node = (node as Record<string | number, unknown>)[step]
    }
    const last = path.at(-1) ?? ''
    ;(node as Record<string | number, unknown>)[last] = value
    expect(parseFrame(reply({ forecast }))).toMatchObject({
      ok: false,
      error: { kind: 'bad_shape' },
    })
  })
})
