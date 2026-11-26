/** Replays for `/dev/replay`: a recorded duel and a four-seat Commander board from the dev table's fixture. */
import { buildFixture } from '@/features/table/dev/fixture'
import type { EventBatch, Replay } from '@/protocol/generated'
import { lobbyStart } from '@/test-utils/transcripts'

/** Text only the dev replay page holds; `devBuild.test.ts` checks that no production file contains it. */
export const REPLAY_FIXTURE_MARKER = 'replay-fixture'

/** The recorded two-seat game of the protocol fixture, from the coin toss to the third turn, seen from seat 0. */
export function duelReplay(): Replay {
  const steps = lobbyStart.messages.flatMap((message) => (message.type === 'Events' ? [message] : []))
  return { seat: 0, names: ['Ada', 'Bo'], result: { Winner: 0 }, steps }
}

/** How many steps the Commander replay has: the board, then one life lost by each opponent in turn. */
export const COMMANDER_STEPS = 6

/** The dev table's four-seat board as a replay: the first step draws it whole, each later step costs an opponent 3 life. */
export function commanderReplay(): Replay {
  const { view, names } = buildFixture({ seats: 4 })
  const { objects, ...table } = view
  const steps: EventBatch[] = [{ from_seq: 0, events: [], table, objects, gone: [] }]
  let players = table.players
  for (let step = 1; step < COMMANDER_STEPS; step += 1) {
    const hit = 1 + ((step - 1) % (players.length - 1))
    players = players.map((player, seat) => (seat === hit ? { ...player, life: player.life - 3 } : player))
    steps.push({ from_seq: 0, events: [], table: { ...table, players }, objects: [], gone: [] })
  }
  return { seat: 0, names, result: null, steps }
}
