// Goal: the counts on the side tabs say what was written since the reader last looked: log lines after a position in the server's log without the quiet ones (passes, steps, mana), chat lines of other seats after the last line read (all of them once that line has left the buffer, none of the reader's own), and a count is worded as its number up to 99 and "99+" past it.
import { describe, expect, it } from 'vitest'
import type { ChatLine, LogEntry } from '@/play/store'
import type { EventView } from '@/protocol/generated'
import { COUNT_CAP, countLabel, unreadChat, unreadLog } from './sideCounts'

const turn = (number: number): EventView => ({ Public: { TurnBegan: { number, active: 0 } } })
const pass = (player: number): EventView => ({ Public: { PriorityPassed: { player } } })

/** Log entries at the positions given, each of a kind that is worth reading ('turn') or quiet ('pass'). */
function logOf(...kinds: Array<['turn' | 'pass', number]>): LogEntry[] {
  return kinds.map(([kind, seq]) => ({ seq, event: kind === 'turn' ? turn(seq) : pass(0) }))
}

describe('countLabel', () => {
  it.each([
    [1, '1'],
    [9, '9'],
    [COUNT_CAP, '99'],
    [COUNT_CAP + 1, '99+'],
    [500, '99+'],
  ])('words %i as %s', (count, label) => {
    expect(countLabel(count)).toBe(label)
  })
})

describe('unreadLog', () => {
  it('counts the lines after the position that was read', () => {
    const log = logOf(['turn', 1], ['turn', 2], ['turn', 3], ['turn', 4])
    expect(unreadLog(log, 0)).toBe(4)
    expect(unreadLog(log, 2)).toBe(2)
    expect(unreadLog(log, 4)).toBe(0)
    expect(unreadLog(log, 9)).toBe(0)
  })

  it('leaves out the quiet lines, such as a pass of priority', () => {
    const log = logOf(['turn', 1], ['pass', 2], ['pass', 3], ['turn', 4], ['pass', 5])
    expect(unreadLog(log, 0)).toBe(2)
    expect(unreadLog(log, 3)).toBe(1)
    expect(unreadLog(log, 4)).toBe(0)
  })

  it('counts by position, so lines that left a full buffer change nothing', () => {
    // The buffer holds only the newest lines; the reader had read up to 10.
    const log = logOf(['turn', 498], ['turn', 499], ['turn', 500])
    expect(unreadLog(log, 10)).toBe(3)
    expect(unreadLog(log, 499)).toBe(1)
  })

  it('has nothing to count in an empty log', () => {
    expect(unreadLog([], 0)).toBe(0)
  })
})

describe('unreadChat', () => {
  const say = (from: number, text: string): ChatLine => ({ from, text })
  const lines = [say(1, 'one'), say(0, 'mine'), say(2, 'two'), say(1, 'three')]

  it('counts what other seats said after the last line read, and not what the reader said', () => {
    expect(unreadChat(lines, null, 0)).toBe(3)
    expect(unreadChat(lines, lines[0] ?? null, 0)).toBe(2)
    expect(unreadChat(lines, lines[2] ?? null, 0)).toBe(1)
    expect(unreadChat(lines, lines[3] ?? null, 0)).toBe(0)
  })

  it('counts every line once the last one read has left the buffer', () => {
    const gone = say(1, 'one')
    // Equal words are another line: lines are told apart by identity, since two seats may say the same thing.
    expect(unreadChat(lines, gone, 0)).toBe(3)
  })

  it('counts every line for a client that holds no seat', () => {
    expect(unreadChat(lines, null, null)).toBe(4)
  })

  it('has nothing to count with nothing said', () => {
    expect(unreadChat([], null, 0)).toBe(0)
  })
})
