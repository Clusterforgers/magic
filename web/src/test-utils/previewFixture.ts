import { isCount, isRecord, isText } from '@/net/check'
import { parseMessage } from '@/net/parse'
import fixtureJson from '@/protocol/fixtures/block-preview.json'
import type { Answer, ClientMessage, DecisionId, ServerMessage } from '@/protocol/generated'

type Of<T extends ServerMessage['type']> = Extract<ServerMessage, { type: T }>

/** One `Preview` the seat sends and the `Forecast` the engine answers it with. */
export interface Exchange {
  label: string
  /** The answer previewed. */
  answer: Answer
  /** The message as it travels. */
  preview: ClientMessage
  /** The reply as it was read at the boundary, and as plain JSON. */
  reply: Of<'Forecast'>
  rawReply: unknown
}

/** A pending decision and the previews made of it. */
export interface Scene {
  seat: number
  /** The id the server gave the decision. */
  id: DecisionId
  decision: Of<'Decision'>
  /** The decision as plain JSON, for sending it to a session. */
  rawDecision: unknown
  exchanges: Exchange[]
}

function readScene(name: string, value: unknown): Scene {
  if (!isRecord(value) || !isCount(value.seat) || !Array.isArray(value.exchanges)) {
    throw new Error(`preview fixture ${name} is malformed`)
  }
  const decision = parseMessage(value.decision)
  if (!decision.ok || decision.message.type !== 'Decision') {
    throw new Error(`preview fixture ${name} does not open with a decision`)
  }
  const exchanges = value.exchanges.map((item: unknown, index): Exchange => {
    if (!isRecord(item) || !isText(item.label) || !isRecord(item.preview)) {
      throw new Error(`preview fixture ${name} exchange ${index} is malformed`)
    }
    const reply = parseMessage(item.reply)
    if (!reply.ok || reply.message.type !== 'Forecast') {
      throw new Error(`preview fixture ${name} exchange ${index}: the reply is no forecast`)
    }
    const { preview } = item
    if (preview.type !== 'Preview' || !isCount(preview.decision) || !isRecord(preview.answer)) {
      throw new Error(`preview fixture ${name} exchange ${index}: not a preview`)
    }
    // The generated types describe the Rust side exactly; the fixture is written by it.
    const answer = preview.answer as Answer
    return {
      label: item.label,
      answer,
      preview: { type: 'Preview', decision: preview.decision, answer },
      reply: reply.message,
      rawReply: item.reply,
    }
  })
  return {
    seat: value.seat,
    id: decision.message.id,
    decision: decision.message,
    rawDecision: value.decision,
    exchanges,
  }
}

const fixture: unknown = fixtureJson
if (!isRecord(fixture)) throw new Error('preview fixture is malformed')

/** Player 1 facing three attackers: the blocks it might make and what the engine says of each. */
export const blocksScene: Scene = readScene('blocks', fixture.blocks)
/** Player 0 declaring them: the attacks it might make and what the engine says of each. */
export const attacksScene: Scene = readScene('attacks', fixture.attacks)
