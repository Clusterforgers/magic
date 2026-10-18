// Goal: a session asks the engine about a choice only for its pending decision, remembers which answer each question was about, keeps only the reply to the newest question while its decision is pending, drops it when the decision is answered, replaced or lost with the connection, and stops asking a server that does not know previews; the real exchanges recorded from the engine replay to the same forecasts.
import { describe, expect, it } from 'vitest'
import type { Answer, CombatForecast } from '@/protocol/generated'
import { attacksScene, blocksScene } from '@/test-utils/previewFixture'
import type { Scene } from '@/test-utils/previewFixture'
import { startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'
import { lateJoin } from '@/test-utils/transcripts'
import { previewKey } from './preview'

const BLOCK: Answer = { DeclareBlockers: [[63, 30]] }
const OTHER: Answer = {
  DeclareBlockers: [
    [63, 30],
    [64, 30],
  ],
}
const FORECAST: CombatForecast = {
  attackers: [],
  creatures: [{ object: 30, damage: 2, dies: true }],
  players: [],
}

const decision = (id: number) => ({
  type: 'Decision',
  id,
  decision: { player: 0, kind: { DeclareBlockers: { blockers: [{ object: 63, can_block: [30] }] } } },
})
const forecast = (id: number, found: CombatForecast | null) => ({
  type: 'Forecast',
  decision: id,
  forecast: found,
})

/** A seat facing the block decision 12 on an open socket. */
function facing(id = 12) {
  const h = startSession()
  h.join()
  h.deliver(decision(id))
  return h
}

describe('asking for a preview', () => {
  it('sends a Preview under the pending decision id, with the answer untouched', () => {
    const h = facing(12)
    expect(h.session.preview(12, BLOCK)).toBe(true)
    expect(h.sentOf('Preview')).toEqual([{ type: 'Preview', decision: 12, answer: BLOCK }])
    expect(h.state().pending?.id).toBe(12)
  })

  it('leaves the decision pending: a preview is not an answer', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    expect(h.state().pending?.id).toBe(12)
    expect(h.session.answer(BLOCK)).toBe(true)
    expect(h.answeredIds()).toEqual([12])
  })

  it.each([
    ['another decision than the pending one', 13],
    ['a decision that was never issued', 0],
  ])('refuses %s without sending anything', (_what, id) => {
    const h = facing(12)
    expect(h.session.preview(id, BLOCK)).toBe(false)
    expect(h.sentOf('Preview')).toEqual([])
  })

  it('refuses when there is no decision yet, after the answer and when the game is over', () => {
    const h = startSession()
    h.join()
    expect(h.session.preview(12, BLOCK)).toBe(false)
    h.deliver(decision(12))
    h.session.answer(BLOCK)
    expect(h.session.preview(12, BLOCK)).toBe(false)
    h.deliver(decision(13), { type: 'GameOver', result: 'Draw' })
    expect(h.session.preview(13, BLOCK)).toBe(false)
    expect(h.sentOf('Preview')).toEqual([])
  })

  it('refuses when the connection is lost or was never opened', () => {
    const h = facing(12)
    h.net.last.fail()
    expect(h.session.preview(12, BLOCK)).toBe(false)
    const fresh = startSession()
    expect(fresh.session.preview(1, BLOCK)).toBe(false)
  })
})

describe('the reply', () => {
  it('is kept with the decision, the key of the answer asked about and the forecast', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.deliver(forecast(12, FORECAST))
    expect(h.state().preview).toEqual({ decision: 12, key: previewKey(BLOCK), forecast: FORECAST })
  })

  it('may hold no forecast, which says the engine had none, not that nothing came', () => {
    const h = facing(12)
    expect(h.state().preview).toBeNull()
    h.session.preview(12, BLOCK)
    h.deliver(forecast(12, null))
    expect(h.state().preview).toEqual({ decision: 12, key: previewKey(BLOCK), forecast: null })
  })

  it('tells answers apart by their key, order of choice included', () => {
    const swapped: Answer = {
      DeclareBlockers: [
        [64, 30],
        [63, 30],
      ],
    }
    expect(previewKey(OTHER)).not.toBe(previewKey(swapped))
    expect(previewKey(BLOCK)).toBe(previewKey({ DeclareBlockers: [[63, 30]] }))
  })

  it('counts only for the newest question: the replies to earlier ones come first and are ignored', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.session.preview(12, OTHER)
    h.deliver(forecast(12, null))
    expect(h.state().preview).toBeNull()
    h.deliver(forecast(12, FORECAST))
    expect(h.state().preview).toEqual({ decision: 12, key: previewKey(OTHER), forecast: FORECAST })
  })

  it('belongs to the question it answers even when a newer one is asked after it arrived', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.deliver(forecast(12, FORECAST))
    h.session.preview(12, OTHER)
    // The older reply is still what is stored, under its own key, until the newer one comes.
    expect(h.state().preview?.key).toBe(previewKey(BLOCK))
    h.deliver(forecast(12, null))
    expect(h.state().preview).toEqual({ decision: 12, key: previewKey(OTHER), forecast: null })
  })

  it('is ignored when nobody asked', () => {
    const h = facing(12)
    h.deliver(forecast(12, FORECAST))
    expect(h.state().preview).toBeNull()
    h.session.preview(12, BLOCK)
    h.deliver(forecast(12, FORECAST), forecast(12, null))
    expect(h.state().preview).toEqual({ decision: 12, key: previewKey(BLOCK), forecast: FORECAST })
  })

  it('is ignored once its decision is not the pending one', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.deliver(decision(13))
    h.deliver(forecast(12, FORECAST))
    expect(h.state().preview).toBeNull()
    // The question after it is answered in order and kept.
    h.session.preview(13, BLOCK)
    h.deliver(forecast(13, FORECAST))
    expect(h.state().preview).toMatchObject({ decision: 13 })
  })

  it('does not count when it names another decision than the question was about', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.deliver(forecast(99, FORECAST))
    expect(h.state().preview).toBeNull()
  })
})

describe('the preview in the store', () => {
  function previewed() {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.deliver(forecast(12, FORECAST))
    expect(h.state().preview).not.toBeNull()
    return h
  }

  it('goes when the decision is answered', () => {
    const h = previewed()
    h.session.answer(BLOCK)
    expect(h.state().preview).toBeNull()
  })

  it('goes when the server replaces the decision', () => {
    const h = previewed()
    h.deliver(decision(13))
    expect(h.state().preview).toBeNull()
  })

  it('goes with a snapshot, which also drops the decision', () => {
    const h = previewed()
    h.deliver(lateJoin.raw[1])
    expect(h.state().pending).toBeNull()
    expect(h.state().preview).toBeNull()
  })

  it('goes when the game ends', () => {
    const h = previewed()
    h.deliver({ type: 'GameOver', result: 'Draw' })
    expect(h.state().preview).toBeNull()
  })

  it('goes when the connection is lost, and a reply on the new connection is not matched to a question of the old', () => {
    const h = previewed()
    h.session.preview(12, OTHER)
    h.net.last.fail()
    expect(h.state().preview).toBeNull()
    h.timers.advance(1000)
    h.open()
    h.deliver(welcome, decision(12))
    h.deliver(forecast(12, FORECAST))
    expect(h.state().preview).toBeNull()
    expect(h.session.preview(12, BLOCK)).toBe(true)
    h.deliver(forecast(12, FORECAST))
    expect(h.state().preview).toMatchObject({ key: previewKey(BLOCK) })
  })
})

describe('a server that predates previews', () => {
  const refusal = {
    type: 'Error',
    code: 'BadMessage',
    message: 'send each message as one JSON text frame of a known type',
  }

  it('answers Preview with BadMessage: no notice, and no more questions', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.deliver(refusal)
    expect(h.state().notice).toBeNull()
    expect(h.state().preview).toBeNull()
    expect(h.session.preview(12, OTHER)).toBe(false)
    expect(h.sentOf('Preview')).toHaveLength(1)
  })

  it('also swallows the refusals of the questions already on their way', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.session.preview(12, OTHER)
    h.deliver(refusal, refusal)
    expect(h.state().notice).toBeNull()
    // The next refusal has no question behind it, so it is news.
    h.deliver(refusal)
    expect(h.state().notice).toContain('did not accept')
  })

  it('is asked again on a new connection, which may be to a newer server', () => {
    const h = facing(12)
    h.session.preview(12, BLOCK)
    h.deliver(refusal)
    h.net.last.fail()
    h.timers.advance(1000)
    h.open()
    h.deliver(welcome, decision(12))
    expect(h.session.preview(12, BLOCK)).toBe(true)
  })

  it('still reports a refusal that no question explains', () => {
    const h = facing(12)
    h.deliver(refusal)
    expect(h.state().notice).toContain('did not accept')
  })
})

const scenes: { name: string; scene: Scene; seat: number }[] = [
  { name: 'blocks', scene: blocksScene, seat: 1 },
  { name: 'attacks', scene: attacksScene, seat: 0 },
]

describe.each(scenes)('the recorded $name exchanges', ({ scene, seat }) => {
  it.each(scene.exchanges.map((exchange) => [exchange.label, exchange] as const))(
    '%s: the question goes out as recorded and the reply is stored as the engine gave it',
    (_label, exchange) => {
      const h = startSession()
      h.join(seat, 2)
      h.deliver(scene.rawDecision)
      expect(h.state().pending?.id).toBe(scene.id)
      expect(h.session.preview(scene.id, exchange.answer)).toBe(true)
      expect(h.sentOf('Preview')).toEqual([exchange.preview])
      h.deliver(exchange.rawReply)
      expect(h.state().preview).toEqual({
        decision: scene.id,
        key: previewKey(exchange.answer),
        forecast: exchange.reply.forecast,
      })
    },
  )

  it('holds forecasts and refusals both', () => {
    const forecasts = scene.exchanges.filter((exchange) => exchange.reply.forecast !== null)
    expect(forecasts.length).toBeGreaterThanOrEqual(3)
    expect(forecasts.length).toBeLessThan(scene.exchanges.length)
  })
})
