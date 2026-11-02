// Goal: the preferences client loads and replaces the account's preferences with the session cookie, reads only well-formed answers, and turns every failure into a PrefsError.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'
import { PrefsError, createPrefsApi } from './prefs'

function serve(answer: (call: FetchCall) => Response | Promise<Response>) {
  const fake = fakeFetch(answer)
  return { api: createPrefsApi({ fetch: fake.fetch }), calls: fake.calls }
}

const stored = {
  default_stops: [{ side: 'Own', step: 'Upkeep', keep: true }],
  end_turn_by_hand: true,
  auto_choices: { order_triggers: true, assign_damage: false, smart_stops: true },
}

describe('createPrefsApi', () => {
  it('loads with the cookie and saves with a PUT of the preferences', async () => {
    const { api, calls } = serve(() => jsonResponse(200, stored))
    expect(await api.load()).toEqual(stored)
    expect(await api.save(stored as never)).toEqual(stored)
    expect(calls.map((call) => [call.url, call.init?.method])).toEqual([
      ['/api/me/prefs', undefined],
      ['/api/me/prefs', 'PUT'],
    ])
    expect(calls[0]?.init?.credentials).toBe('include')
    expect(JSON.parse(String(calls[1]?.init?.body))).toEqual(stored)
  })

  it('reads auto choices without smart_stops (an older server) as smart stops off', async () => {
    const older = { ...stored, auto_choices: { order_triggers: true, assign_damage: false } }
    const { api } = serve(() => jsonResponse(200, older))
    expect(await api.load()).toEqual({
      ...stored,
      auto_choices: { ...older.auto_choices, smart_stops: false },
    })
  })

  it.each([
    ['not an object', 'x'],
    ['no flag', { default_stops: [] }],
    [
      'stops of another kind',
      { default_stops: [1], end_turn_by_hand: false, auto_choices: stored.auto_choices },
    ],
    ['auto choices missing', { default_stops: [], end_turn_by_hand: false }],
    [
      'auto choices of another kind',
      { default_stops: [], end_turn_by_hand: false, auto_choices: { order_triggers: 1 } },
    ],
  ])('refuses an answer that is %s', async (_, body) => {
    const { api } = serve(() => jsonResponse(200, body))
    await expect(api.load()).rejects.toBeInstanceOf(PrefsError)
  })

  it('rejects a refusal with its status and no answer with status 0', async () => {
    const refused = serve(() => jsonResponse(401, { error: 'unauthenticated', message: 'no' }))
    await expect(refused.api.load()).rejects.toMatchObject({ status: 401 })
    const down = serve(() => Promise.reject(new TypeError('offline')))
    await expect(down.api.load()).rejects.toMatchObject({ status: 0 })
  })
})
