/** The start of the next turn for `?newturn=`: the batch that makes the turn badge, the banner and the ticker change, to look at the turn cues. */
import type { PlayState } from '@/play/store'
import type { EventBatch } from '@/protocol/generated'
import { tableOf } from './fxScenarios'

/**
 * The batch that passes the turn to the other side: the opponent after the viewer's turn, the viewer after an opponent's.
 * Null without a table. It builds the table of the next turn and a `TurnBegan` event, as the server sends them.
 */
export function turnChangeBatch(state: PlayState): EventBatch | null {
  const { view, seq } = state
  if (view === null || seq === null) return null
  const next = structuredClone(view)
  const active = next.turn.active === next.viewer ? (next.viewer + 1) % next.players.length : next.viewer
  next.turn = { ...next.turn, number: next.turn.number + 1, active, step: 'Main1', lands_played: 0 }
  return {
    from_seq: seq,
    events: [{ Public: { TurnBegan: { number: next.turn.number, active } } }],
    table: tableOf(next),
    objects: [],
    gone: [],
  }
}
