// Goal: the counts on the side tabs say what was written since the reader last looked: chat lines of other seats after the last line read (all of them once that line has left the buffer, none of the reader's own), and a count is worded as its number up to 99 and "99+" past it.
import { describe, expect, it } from 'vitest'
import type { ChatLine } from '@/play/store'
import { COUNT_CAP, countLabel, unreadChat } from './sideCounts'

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
