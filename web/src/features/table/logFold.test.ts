// Goal: the log draws a round of priority as one line: "All pass" with the seats' names when every seat in the game passed and a spell or ability was passed over to resolve, "Bo and Cy pass" when an action cut the round short, nothing at all for a seat receiving priority, for a step that went by with everybody passing or for the bookkeeping of a resolution step, and every other line as it was, for 2 and 4 seats and for a game that has lost a seat; an attachment that moves says from which host, as far as the log has told.
import { describe, expect, it } from 'vitest'
import type { HostRef } from '@/play/hosts'
import type { LogEntry } from '@/play/store'
import type { EventView, PlayerId } from '@/protocol/generated'
import { begin, grant, pass, pub, round, stepRound } from '@/test-utils/turnEvents'
import { foldLog } from './logFold'
import type { FoldedLine } from './logFold'

const NAMES = ['Alice', 'Bo', 'Cy', 'Dee']
const names = { seat: (seat: PlayerId) => NAMES[seat] ?? `Seat ${seat}`, card: (id: number) => `Card ${id}` }

const cast = (player: PlayerId, object: number): EventView => pub({ SpellCast: { player, object } })
const resolved = (object: number): EventView => pub({ SpellResolved: { object } })
const resolving = (object: number): EventView => pub({ ResolutionBegan: { object } })
const life = (player: PlayerId, amount: number): EventView => pub({ LifeLost: { player, amount } })

/** The events as log entries at positions 0, 1, 2 ... */
const logOf = (events: readonly EventView[]): LogEntry[] => events.map((event, seq) => ({ seq, event }))

/** The folded lines of a log, as "text" or "text [seats]" for a line of passes. */
function fold(events: readonly EventView[], seats: readonly PlayerId[]): string[] {
  return foldLog(logOf(events), names, { seats }).map((line: FoldedLine) =>
    line.passed.length === 0 ? line.text : `${line.text} [${line.passed.join(',')}]`,
  )
}

const seatsOf = (count: number): PlayerId[] => Array.from({ length: count }, (_, seat) => seat)

describe.each([2, 4])('the log of a %i seat game', (count) => {
  const seats = seatsOf(count)
  const all = seats.map((seat) => pass(seat))

  it('leaves out a step that went by with everybody passing, however many seats pass', () => {
    const events = [...stepRound('Upkeep', 0, count), ...stepRound('Draw', 0, count)]
    expect(fold(events, seats)).toEqual([])
  })

  it('folds the passes of a spell everybody let resolve into one line of all the seats', () => {
    const events = [cast(0, 7), ...round(0, count), resolved(7)]
    expect(fold(events, seats)).toEqual([
      'Alice casts Card 7',
      `All pass [${seats.join(',')}]`,
      'Card 7 resolves',
    ])
    const [, folded] = foldLog(logOf(events), names, { seats })
    expect(folded?.minor).toBe(true)
    expect(folded?.spoken).toBe(
      `${NAMES.slice(0, count).slice(0, -1).join(', ')} and ${NAMES[count - 1]} pass`,
    )
  })

  it('leaves out the bookkeeping of a resolution step between the lines of what it did', () => {
    const events = [
      resolving(7),
      life(1, 2),
      pub({ StepDone: { position: [1], bound: [], snapshots: [] } }),
      life(1, 1),
    ]
    expect(fold(events, seats)).toEqual(['Card 7 begins to resolve', 'Bo loses 2 life', 'Bo loses 1 life'])
  })

  it('counts a round the same with or without the grants, and a seat that passes twice once', () => {
    const events = [cast(0, 7), ...all, pass(0), resolving(7)]
    expect(fold(events, seats)).toEqual([
      'Alice casts Card 7',
      `All pass [${seats.join(',')}]`,
      'Card 7 begins to resolve',
    ])
  })

  it('keeps the lines that are worth reading, in order, between the folds', () => {
    const events = [
      begin('DeclareAttackers'),
      ...round(0, count),
      cast(0, 7),
      ...round(0, count),
      resolved(7),
      life(1, 3),
      ...round(0, count),
      begin('End'),
    ]
    expect(fold(events, seats)).toEqual([
      'Declare attackers',
      'Alice casts Card 7',
      `All pass [${seats.join(',')}]`,
      'Card 7 resolves',
      'Bo loses 3 life',
    ])
  })

  it('waits for the event after a round before it says what the round was', () => {
    const events = [cast(0, 7), ...round(0, count)]
    expect(fold(events, seats)).toEqual(['Alice casts Card 7'])
    expect(fold([...events, resolved(7)], seats)).toHaveLength(3)
  })
})

describe('a round an action cut short', () => {
  it('names the seats that passed before it, in order, and the seats that passed after it', () => {
    const seats = seatsOf(4)
    const events = [
      cast(0, 7),
      pass(1),
      pass(2),
      cast(3, 8),
      ...seats.map((seat) => pass(seat)),
      resolving(8),
    ]
    expect(fold(events, seats)).toEqual([
      'Alice casts Card 7',
      'Bo and Cy pass [1,2]',
      'Dee casts Card 8',
      'All pass [0,1,2,3]',
      'Card 8 begins to resolve',
    ])
  })

  it('says "passes" for one seat', () => {
    expect(fold([cast(0, 7), pass(1), cast(0, 8)], seatsOf(2))).toEqual([
      'Alice casts Card 7',
      'Bo passes [1]',
      'Alice casts Card 8',
    ])
  })

  it('drops a grant that was not followed by a pass', () => {
    expect(fold([grant(1), cast(1, 7)], seatsOf(2))).toEqual(['Bo casts Card 7'])
  })
})

describe('a game that has lost a seat', () => {
  it('calls the round of the seats left "All pass"', () => {
    const left = [0, 1, 3]
    const events = [cast(0, 7), pass(0), pass(1), pass(3), resolved(7)]
    expect(fold(events, left)).toEqual(['Alice casts Card 7', 'All pass [0,1,3]', 'Card 7 resolves'])
  })

  it('does not call it "All pass" while a seat that is left still has to pass', () => {
    const events = [cast(0, 7), pass(0), pass(1), resolved(7)]
    expect(fold(events, [0, 1, 3])).toEqual([
      'Alice casts Card 7',
      'Alice and Bo pass [0,1]',
      'Card 7 resolves',
    ])
  })
})

describe('a long log', () => {
  it('folds the eight lines a step takes with four seats, and the step with them, to nothing, and keeps one line for each spell passed', () => {
    const steps = ['Upkeep', 'Draw', 'Main1', 'BeginCombat', 'DeclareAttackers', 'End'] as const
    const events = steps.flatMap((step) => stepRound(step, 0, 4))
    events.splice(9, 0, cast(0, 1), ...round(0, 4), resolved(1))
    const lines = fold(events, seatsOf(4))
    expect(events.length).toBeGreaterThan(50)
    expect(lines).toEqual(['Upkeep', 'Alice casts Card 1', 'All pass [0,1,2,3]', 'Card 1 resolves'])
    expect(lines.filter((line) => line.startsWith('All pass'))).toHaveLength(1)
    expect(lines.some((line) => line.includes('priority'))).toBe(false)
  })
})

describe('an attachment in the log', () => {
  const attach = (object: number, to: number, hostBefore?: HostRef | null): LogEntry => ({
    seq: object * 10 + to,
    event: pub({ Attached: { object, to } }),
    ...(hostBefore === undefined ? {} : { hostBefore }),
  })
  const lines = (log: readonly LogEntry[]): string[] =>
    foldLog(log, names, { seats: seatsOf(2) }).map((line) => line.text)

  it.each<[string, HostRef | null | undefined, string]>([
    ['from a creature to another', { card: 1 }, 'Card 7 moves from Card 1 to Card 2'],
    ['from a player to a creature', { player: 1 }, 'Card 7 moves from Bo to Card 2'],
    ['from nothing', null, 'Card 7 is attached to Card 2'],
    ['from a host the log was not told of', undefined, 'Card 7 becomes attached to Card 2'],
  ])('says an attachment that moves %s', (_name, hostBefore, text) => {
    expect(lines([attach(7, 2, hostBefore)])).toEqual([text])
  })

  it('says a move to a player and an unattach in their usual words', () => {
    const onPlayer: LogEntry = {
      seq: 1,
      event: pub({ AttachedToPlayer: { object: 7, player: 1 } }),
      hostBefore: { card: 1 },
    }
    const away: LogEntry = { seq: 2, event: pub({ Unattached: { object: 7 } }) }
    expect(lines([onPlayer, away])).toEqual(['Card 7 moves from Card 1 to Bo', 'Card 7 becomes unattached'])
  })
})
