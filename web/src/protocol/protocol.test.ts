// Goal: the generated protocol types accept the JSON shapes the Rust side emits and narrow on the `type` tag.
import { describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION } from './generated'
import type { ClientMessage, ServerMessage } from './generated'

const describeServer = (message: ServerMessage): string => {
  switch (message.type) {
    case 'Decision':
      return `decision ${message.id} for player ${message.decision.player}`
    case 'Events':
      return `${message.events.length} events from ${message.from_seq}`
    case 'Snapshot':
      return `snapshot of ${message.state.players.length} players at ${message.seq}`
    default:
      return message.type
  }
}

describe('protocol types', () => {
  it('builds a join message with the generated version constant', () => {
    const join: ClientMessage = { type: 'Join', protocol_version: PROTOCOL_VERSION, game: 'g', name: 'Ana' }
    expect(JSON.parse(JSON.stringify(join))).toEqual({
      type: 'Join',
      protocol_version: 1,
      game: 'g',
      name: 'Ana',
    })
  })

  it('narrows server messages parsed from wire JSON', () => {
    const wire =
      '{"type":"Decision","id":8,"decision":{"player":1,"kind":{"Discard":{"hand":[3,4],"count":1}}}}'
    const message = JSON.parse(wire) as ServerMessage
    expect(describeServer(message)).toBe('decision 8 for player 1')
  })

  it('answers a decision with an externally tagged engine answer', () => {
    const answer: ClientMessage = { type: 'Answer', decision: 8, answer: { Discard: [3] } }
    expect(answer.type).toBe('Answer')
  })
})
