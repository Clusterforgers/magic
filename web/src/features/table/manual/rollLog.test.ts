// Goal: a coin flipped or a die rolled by hand is one log line for every viewer, "Bo rolls a d20: 14" or "Bo flips a coin: heads", the action and its result told once, with the lines around it kept; a result without its action reads on its own.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { Event, EventView, ManualRoll, PlayerId } from '@/protocol/generated'
import { pub } from '@/test-utils/turnEvents'
import { describeEvent } from '../eventText'
import { foldLog } from '../logFold'

const NAMES = ['Ana', 'Bo']
const names = {
  seat: (seat: PlayerId) => NAMES[seat] ?? `Seat ${seat}`,
  card: () => 'a card',
}

const roll = (kind: ManualRoll, player: PlayerId = 1): EventView =>
  pub({ ManualAction: { player, action: { Roll: { kind } } } })
const rolled = (kind: ManualRoll, result: number, player: PlayerId = 1): EventView =>
  pub({ ManualRolled: { player, kind, result } })
const fold = (events: readonly EventView[]): string[] =>
  foldLog(
    events.map((event, seq): LogEntry => ({ seq, event })),
    names,
    { seats: [0, 1] },
  ).map((line) => line.text)

describe('a roll by hand in the log', () => {
  it.each<[ManualRoll, number, string]>([
    [{ Die: { sides: 20 } }, 14, 'Bo rolls a d20: 14'],
    [{ Die: { sides: 6 } }, 6, 'Bo rolls a d6: 6'],
    [{ Die: { sides: 37 } }, 1, 'Bo rolls a d37: 1'],
    [{ Coin: {} }, 1, 'Bo flips a coin: heads'],
    [{ Coin: {} }, 2, 'Bo flips a coin: tails'],
  ])('%j then %s is the one line "%s"', (kind, result, text) => {
    expect(fold([roll(kind), rolled(kind, result)])).toEqual([text])
  })

  it('keeps the lines before and after and tells two rolls apart', () => {
    const d20: ManualRoll = { Die: { sides: 20 } }
    const lines = fold([
      pub({ LibraryShuffled: { player: 0, order: [] } }),
      roll(d20, 0),
      rolled(d20, 3, 0),
      roll({ Coin: {} }),
      rolled({ Coin: {} }, 2),
    ])
    expect(lines).toEqual(['Ana shuffles their library', 'Ana rolls a d20: 3', 'Bo flips a coin: tails'])
  })

  it('reads a result on its own', () => {
    const event: Event = { ManualRolled: { player: 0, kind: { Die: { sides: 12 } }, result: 5 } }
    expect(describeEvent(pub(event), names).text).toBe('Ana rolls a d12: 5')
  })
})
