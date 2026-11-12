// Goal: the tournament client sends each call to its path with its method and body, carries the cookie, and turns every failure into a TournamentError with the server's code, or its own for a lost connection and an unreadable answer.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { createTournamentsApi, TournamentError } from './tournaments'
import type { TournamentsApi } from './tournaments'

const VIEW = {
  id: 't1',
  name: 'Friday',
  format: 'commander_1v1',
  structure: 'swiss',
  status: 'running',
  round: 1,
  rounds: 3,
  organiser_name: 'Org',
  organiser: true,
  invite: 'ABCDEFGH',
  playable: true,
  entrants: [],
  rounds_played: [],
  standings: [],
}

function client(status = 200, body: unknown = VIEW) {
  const fake = fakeFetch(() => (status === 204 ? new Response(null, { status }) : jsonResponse(status, body)))
  return { api: createTournamentsApi({ fetch: fake.fetch }), calls: fake.calls }
}

describe('the tournament client', () => {
  it.each<[string, (api: TournamentsApi) => Promise<unknown>, string, string, unknown]>([
    ['get', (api) => api.get('t1'), 'GET', '/api/tournaments/t1', undefined],
    ['start', (api) => api.act('t1', 'start'), 'POST', '/api/tournaments/t1/start', undefined],
    ['next', (api) => api.act('t1', 'next'), 'POST', '/api/tournaments/t1/next', undefined],
    ['drop out', (api) => api.act('t1', 'drop'), 'POST', '/api/tournaments/t1/drop', undefined],
    ['drop', (api) => api.dropEntrant('t1', 2), 'POST', '/api/tournaments/t1/entrants/2/drop', undefined],
    [
      'result',
      (api) => api.setOutcome('t1', 2, 3, 'draw'),
      'PUT',
      '/api/tournaments/t1/matches/2/3',
      { outcome: 'draw' },
    ],
    [
      'join',
      (api) => api.join('AB CD', { team: 1 }),
      'POST',
      '/api/tournaments/invite/AB%20CD/join',
      { team: 1 },
    ],
  ])('%s', async (_, call, method, path, body) => {
    const { api, calls } = client()
    await expect(call(api)).resolves.toMatchObject({ id: 't1' })
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(path)
    expect(calls[0]?.init?.method ?? 'GET').toBe(method)
    expect(calls[0]?.init?.credentials).toBe('include')
    const sent = calls[0]?.init?.body
    expect(sent === undefined ? undefined : JSON.parse(String(sent))).toEqual(body)
  })

  it('creates a tournament and reads back its invite', async () => {
    const { api, calls } = client(201, { id: 't9', invite: 'ABCDEFGH' })
    const made = await api.create({ name: 'Cup', format: 'commander_1v1', structure: 'elimination' })
    expect(made).toEqual({ id: 't9', invite: 'ABCDEFGH' })
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual({
      name: 'Cup',
      format: 'commander_1v1',
      structure: 'elimination',
    })
  })

  it('takes an empty answer for leave and remove', async () => {
    const { api, calls } = client(204)
    await api.leave('t1')
    await api.remove('t1')
    expect(calls.map((c) => [c.init?.method, c.url])).toEqual([
      ['POST', '/api/tournaments/t1/leave'],
      ['DELETE', '/api/tournaments/t1'],
    ])
  })

  it.each([
    [409, { error: 'undecided', message: 'a match has no result' }, 'undecided'],
    [403, { error: 'forbidden', message: 'only the organiser' }, 'forbidden'],
    [418, { error: 'teapot', message: 'odd' }, 'unknown'],
    [500, 'not json', 'bad_response'],
  ])('turns HTTP %i into a TournamentError', async (status, body, code) => {
    const { api } = client(status, body)
    const failure = await api.act('t1', 'next').catch((e: unknown) => e)
    expect(failure).toBeInstanceOf(TournamentError)
    expect((failure as TournamentError).code).toBe(code)
    expect((failure as TournamentError).status).toBe(status)
  })

  it('refuses an answer of the wrong shape', async () => {
    const { api } = client(200, { id: 't1' })
    await expect(api.get('t1')).rejects.toMatchObject({ code: 'bad_response' })
  })

  it('names a lost connection', async () => {
    const api = createTournamentsApi({ fetch: () => Promise.reject(new TypeError('offline')) })
    await expect(api.list()).rejects.toMatchObject({ code: 'network', status: 0 })
  })
})
