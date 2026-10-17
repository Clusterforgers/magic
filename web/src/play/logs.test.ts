// Goal: the event log and the chat keep only their newest lines, in order and with the server's positions, and chat input is trimmed and bounded the way the server bounds it.
import { describe, expect, it } from 'vitest'
import type { EventView } from '@/protocol/generated'
import { batchOf, must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { lateJoin } from '@/test-utils/transcripts'
import { CHAT_LIMIT, LOG_LIMIT } from './store'
import { MAX_CHAT_CHARS } from './session'

const snapshot = must(
  lateJoin.messages.find((message) => message.type === 'Snapshot'),
  'snapshot',
)
const base = snapshot.state

/** `count` distinct events, numbered by the seat they name so a test can tell them apart. */
const events = (from: number, count: number): EventView[] =>
  Array.from({ length: count }, (_, i): EventView => ({ Public: { PriorityPassed: { player: from + i } } }))

/** The batch message that continues the log at `fromSeq`, leaving the table as the snapshot had it. */
const batch = (fromSeq: number, count: number) => ({
  type: 'Events',
  ...batchOf(base, { from_seq: fromSeq, events: events(fromSeq, count) }),
})

function started() {
  const h = startSession()
  h.join()
  h.deliver(snapshot)
  return h
}

describe('the event log', () => {
  it('starts empty at a Snapshot and fills with positions that follow the snapshot', () => {
    const h = started()
    expect(h.state().log).toEqual([])
    h.deliver(batch(snapshot.seq, 3))
    expect(h.state().log.map((entry) => entry.seq)).toEqual([
      snapshot.seq,
      snapshot.seq + 1,
      snapshot.seq + 2,
    ])
    expect(h.state().log.map((entry) => entry.event)).toEqual(events(snapshot.seq, 3))
    expect(h.state().seq).toBe(snapshot.seq + 3)
  })

  it.each([
    ['one batch of 1200', [1200]],
    ['two batches of 300', [300, 300]],
    ['seven batches of 100', [100, 100, 100, 100, 100, 100, 100]],
    ['exactly the limit', [LOG_LIMIT]],
    ['one over the limit', [LOG_LIMIT + 1]],
  ])('keeps the newest %s events only, oldest dropped, positions unbroken', (_name, sizes) => {
    const h = started()
    let at = snapshot.seq
    for (const size of sizes) {
      h.deliver(batch(at, size))
      at += size
    }
    const log = h.state().log
    const total = at - snapshot.seq
    expect(log).toHaveLength(Math.min(LOG_LIMIT, total))
    expect(log.at(-1)?.seq).toBe(at - 1)
    expect(log[0]?.seq).toBe(at - log.length)
    expect(log.map((entry) => entry.seq)).toEqual(log.map((_entry, i) => at - log.length + i))
    expect(log.map((entry) => entry.event)).toEqual(events(at - log.length, log.length))
  })

  it('keeps the entries of earlier batches by identity while it grows', () => {
    const h = started()
    h.deliver(batch(snapshot.seq, 2))
    const first = h.state().log
    h.deliver(batch(snapshot.seq + 2, 2))
    expect(
      h
        .state()
        .log.slice(0, 2)
        .every((entry, i) => entry === first[i]),
    ).toBe(true)
  })

  it('leaves the log alone for a batch with no events', () => {
    const h = started()
    h.deliver(batch(snapshot.seq, 2))
    const before = h.state().log
    h.deliver(batch(snapshot.seq + 2, 0))
    expect(h.state().log).toBe(before)
  })

  it('keeps its history across a Snapshot that continues it, and drops what the Snapshot makes untrue', () => {
    const h = started()
    h.deliver(batch(snapshot.seq, 10))
    h.deliver({ type: 'Snapshot', seq: snapshot.seq + 10, state: base })
    expect(h.state().log).toHaveLength(10)
    h.deliver({ type: 'Snapshot', seq: snapshot.seq + 4, state: base })
    expect(h.state().log.map((entry) => entry.seq)).toEqual([0, 1, 2, 3].map((i) => snapshot.seq + i))
    h.deliver({ type: 'Snapshot', seq: 0, state: base })
    expect(h.state().log).toEqual([])
  })
})

describe('incoming chat', () => {
  it('keeps the lines in the order they came, with who said them', () => {
    const h = startSession()
    h.join()
    h.deliver({ type: 'Chat', from: 1, text: 'hi' }, { type: 'Chat', from: 0, text: 'hello' })
    expect(h.state().chat).toEqual([
      { from: 1, text: 'hi' },
      { from: 0, text: 'hello' },
    ])
  })

  it('keeps only the newest lines', () => {
    const h = startSession()
    h.join()
    for (let n = 0; n < CHAT_LIMIT + 50; n += 1) h.deliver({ type: 'Chat', from: n % 2, text: `line ${n}` })
    const chat = h.state().chat
    expect(chat).toHaveLength(CHAT_LIMIT)
    expect(chat[0]?.text).toBe('line 50')
    expect(chat.at(-1)?.text).toBe(`line ${CHAT_LIMIT + 49}`)
  })
})

describe('outgoing chat', () => {
  const sendable: [string, string, string][] = [
    ['plain text', 'good game', 'good game'],
    ['padded text', '  good game \n', 'good game'],
    ['text with inner spaces', 'a  b', 'a  b'],
    ['exactly the longest line', 'a'.repeat(MAX_CHAT_CHARS), 'a'.repeat(MAX_CHAT_CHARS)],
    [
      'the longest line of emoji, which are two UTF-16 units each',
      '😀'.repeat(MAX_CHAT_CHARS),
      '😀'.repeat(MAX_CHAT_CHARS),
    ],
  ]

  it.each(sendable)('sends %s', (_name, text, sent) => {
    const h = startSession()
    h.join()
    expect(h.session.chat(text)).toBe(true)
    expect(h.sentOf('Chat')).toEqual([{ type: 'Chat', text: sent }])
    expect(h.state().notice).toBeNull()
  })

  it.each([
    ['nothing', ''],
    ['spaces', '   '],
    ['whitespace of every kind', ' \t\n\r '],
  ])('does not send %s', (_name, text) => {
    const h = startSession()
    h.join()
    expect(h.session.chat(text)).toBe(false)
    expect(h.sentOf('Chat')).toEqual([])
    expect(h.state().notice).toBeNull()
  })

  it.each([
    ['one character too many', 'a'.repeat(MAX_CHAT_CHARS + 1)],
    ['one emoji too many', '😀'.repeat(MAX_CHAT_CHARS + 1)],
  ])('refuses %s with a notice instead of letting the server refuse it', (_name, text) => {
    const h = startSession()
    h.join()
    expect(h.session.chat(text)).toBe(false)
    expect(h.sentOf('Chat')).toEqual([])
    expect(h.state().notice).toContain(String(MAX_CHAT_CHARS))
  })
})
