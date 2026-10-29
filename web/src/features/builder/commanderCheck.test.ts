// Goal: a candidate commander is judged by the issues of the deck check that are about choosing commanders, a card already in the command zone is refused at once, and a check that fails refuses the card with its reason.
import { describe, expect, it } from 'vitest'
import { DeckError } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type { CheckRequest, DeckReport, Issue } from '@/protocol/generated'
import { reportOf } from '@/features/decks/fakeDecks'
import { commanderReasons, judgeCommander, withCommander } from './commanderCheck'
import { BEAR, KRENKO, TYMNA, commanderLegality } from './commanderFixture'
import type { Draft } from './deckEdit'

const draftWith = (...cards: (typeof KRENKO)[]): Draft => ({
  name: 'Deck',
  format: 'commander',
  notes: '',
  lines: cards.map((card) => ({ zone: 'commander', card, count: 1, printingId: null })),
})

const api = (answer: (request: CheckRequest) => DeckReport | Promise<DeckReport>) =>
  ({ check: async (request: CheckRequest) => answer(request) }) as unknown as DecksApi

const judging = (): { calls: CheckRequest[]; api: DecksApi } => {
  const calls: CheckRequest[] = []
  return {
    calls,
    api: api((request) => {
      calls.push(request)
      return reportOf([], commanderLegality(request), { format: 'commander' })
    }),
  }
}

describe('judgeCommander', () => {
  it('accepts a legend and sends the command zone with it, in the Commander format', async () => {
    const { api: server, calls } = judging()
    expect(await judgeCommander(server, draftWith(), KRENKO)).toEqual({ ok: true })
    expect(calls[0]?.format).toBe('commander')
    expect(calls[0]?.cards?.map((line) => [line.zone, line.oracle_id])).toEqual([
      ['commander', KRENKO.oracle_id],
    ])
  })

  it('refuses with the sentence of the issue about the card', async () => {
    const verdict = await judgeCommander(judging().api, draftWith(), BEAR)
    expect(verdict).toEqual({
      ok: false,
      reason: 'Grizzly Bears cannot be a commander: it is no legendary creature and does not say it can.',
    })
  })

  it('refuses a pair that cannot be partnered', async () => {
    const verdict = await judgeCommander(judging().api, draftWith(KRENKO), TYMNA)
    expect(verdict.ok).toBe(false)
    expect(verdict).toMatchObject({ reason: expect.stringContaining('cannot be partnered') })
  })

  it('refuses a card that already is a commander without asking', async () => {
    const { api: server, calls } = judging()
    const verdict = await judgeCommander(server, draftWith(KRENKO), KRENKO)
    expect(verdict).toEqual({ ok: false, reason: 'Krenko, Mob Boss is already a commander of this deck.' })
    expect(calls).toEqual([])
  })

  it('refuses when the check cannot be made, naming the card', async () => {
    const down = api(() => Promise.reject(new DeckError('network', 0, 'offline')))
    const verdict = await judgeCommander(down, draftWith(), KRENKO)
    expect(verdict).toMatchObject({
      ok: false,
      reason: expect.stringContaining('Could not check Krenko, Mob Boss'),
    })
  })
})

describe('commanderReasons', () => {
  const issue = (code: Issue['code']): Issue => ({
    code,
    message: `${code} problem`,
    zone: null,
    oracle_id: null,
    name: null,
  })

  it('keeps the issues about choosing commanders and drops the rest, in order', () => {
    const issues = (
      ['deck_size', 'commander_pair', 'copies', 'commander_eligibility', 'commander_count'] as const
    ).map(issue)
    expect(commanderReasons(issues)).toEqual([
      'Commander_pair problem.',
      'Commander_eligibility problem.',
      'Commander_count problem.',
    ])
  })

  it('puts one copy of the candidate after the others in the command zone', () => {
    expect(withCommander(draftWith(KRENKO), TYMNA).map((line) => line.card.name)).toEqual([
      KRENKO.name,
      TYMNA.name,
    ])
  })
})
