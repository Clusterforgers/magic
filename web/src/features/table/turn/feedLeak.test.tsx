// Goal: what the turn cues show is the same for two games that differ only in what a seat cannot see: replaying a recorded game and a copy of it in which one hidden card is another card gives identical feeds and the same cues on screen, and replaying it with a seat's passes at the pace of an automatic pass (a short dwell) and at the pace of a seat deciding (seconds) gives the same feeds and the same cue for every batch, so nothing the cues say tells an opponent's hand, or whether it passed by itself.
import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cueDom } from '@/test-utils/cueTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { lateJoin, lobbyStart } from '@/test-utils/transcripts'
import type { Transcript } from '@/test-utils/transcripts'
import { feedBetween } from './feed'
import type { Feed } from './feed'

const NAMES = ['Ana', 'Bo']
const name = (seat: number): string => NAMES[seat] ?? `Seat ${seat}`
const lookup = {
  game: {
    started: true,
    seats: NAMES.map((seatName, seat) => ({ seat, name: seatName })),
    connected: [0, 1],
    ready: [0, 1],
    result: null,
  },
  missing: false,
  failed: false,
}

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  vi.useRealTimers()
})

type Json = Record<string, unknown>
const isJson = (value: unknown): value is Json => typeof value === 'object' && value !== null

/** The id of the first card the viewer is dealt or draws in a transcript, a card only the viewer could have named. */
function hiddenCard(raw: readonly unknown[]): number {
  for (const message of raw) {
    if (!isJson(message) || !Array.isArray(message.events)) continue
    for (const view of message.events as unknown[]) {
      const event = isJson(view) && isJson(view.Public) ? view.Public : null
      const drawn = event !== null && isJson(event.CardDrawn) ? event.CardDrawn : null
      if (drawn !== null && drawn.player === 0 && typeof drawn.object === 'number') return drawn.object
      const dealt = event !== null && isJson(event.HandDealt) ? event.HandDealt : null
      const first = dealt !== null && dealt.player === 0 && Array.isArray(dealt.cards) ? dealt.cards[0] : null
      if (typeof first === 'number') return first
    }
  }
  throw new Error('the transcript has no card dealt or drawn to the viewer')
}

/** A copy of the transcript in which card `id` is another card wherever the viewer is told of it. */
function withCardRenamed(raw: readonly unknown[], id: number, name: string): unknown[] {
  const copy = structuredClone(raw) as unknown[]
  for (const message of copy) {
    if (!isJson(message)) continue
    const lists = [message.objects, isJson(message.state) ? message.state.objects : undefined]
    for (const list of lists) {
      if (!Array.isArray(list)) continue
      for (const object of list as unknown[]) {
        if (isJson(object) && object.id === id) object.name = name
      }
    }
  }
  return copy
}

/** Plays a transcript's messages into a session and gives the feed of every change, null where there is none. */
function replayFeeds(raw: readonly unknown[]): Array<Feed | null> {
  const h = startSession()
  h.session.connect()
  h.open()
  const feeds: Array<Feed | null> = []
  h.session.store.subscribe((next, prev) => feeds.push(feedBetween(prev, next, name)))
  for (const message of raw) h.deliver(message)
  return feeds
}

/**
 * Plays the messages into a table with the cues on, leaving `pauseMs` between them, and notes the cues each batch put
 * on screen at the moment it arrived: the banner and the first ticker line.
 */
function replayCues(
  raw: readonly unknown[],
  pauseMs: number,
): Array<{ banner: string | null; ticker: string | null }> {
  const h = startSession()
  h.session.connect()
  h.open()
  const last: { feed: Feed | null } = { feed: null }
  // Read through a function: the store sets it while a message is delivered, which the compiler cannot follow.
  const batchOf = (): Feed | null => last.feed
  h.session.store.subscribe((next, prev) => {
    last.feed = feedBetween(prev, next, name)
  })
  const { unmount } = renderLiveTable(h.session, lookup, { cues: true })
  const seen: Array<{ banner: string | null; ticker: string | null }> = []
  for (const message of raw) {
    last.feed = null
    act(() => h.deliver(message))
    const feed = batchOf()
    if (feed !== null) {
      const dom = cueDom()
      // A batch with no line leaves the ticker as the last one made it, which depends on how long ago that was.
      seen.push({
        banner: feed.banner === null ? null : dom.banner,
        ticker: feed.lines.length === 0 ? null : dom.ticker,
      })
    }
    act(() => {
      vi.advanceTimersByTime(pauseMs)
    })
  }
  unmount()
  return seen
}

const json = (value: unknown): string => JSON.stringify(value)

describe.each<[string, Transcript]>([
  ['the late join', lateJoin],
  ['the lobby start', lobbyStart],
])('two games of %s that differ in one hidden card', (_name, transcript) => {
  const SECRET = 'Zzyzx Secret Card'
  const id = hiddenCard(transcript.raw)

  it('give the same feeds, though the viewer’s own game tells them apart', () => {
    const other = withCardRenamed(transcript.raw, id, SECRET)
    const a = replayFeeds(transcript.raw)
    const b = replayFeeds(other)
    expect(json(b)).toBe(json(a))
    expect(a.filter((feed) => feed !== null).length).toBeGreaterThan(1)

    // The variants really differ for this seat: at some point its own state holds the card under another name.
    const namesOf = (raw: readonly unknown[]): Set<string | undefined> => {
      const h = startSession()
      h.session.connect()
      h.open()
      const names = new Set<string | undefined>()
      for (const message of raw) {
        h.deliver(message)
        names.add(h.state().objectsById[id]?.name)
      }
      return names
    }
    expect(namesOf(other).has(SECRET)).toBe(true)
    expect(namesOf(transcript.raw).has(SECRET)).toBe(false)
    expect(json(a)).not.toContain(SECRET)
  })

  it('put the same words on screen, and none of them the card', () => {
    const a = replayCues(transcript.raw, 400)
    const b = replayCues(withCardRenamed(transcript.raw, id, SECRET), 400)
    expect(json(b)).toBe(json(a))
    expect(json(b)).not.toContain(SECRET)
    expect(document.body.textContent).not.toContain(SECRET)
  })
})

describe.each<[string, Transcript]>([
  ['the late join', lateJoin],
  ['the lobby start', lobbyStart],
])('a seat’s pass in %s, at an automatic pass’s pace or a thinking seat’s', (_name, transcript) => {
  it('gives the same feeds: the events and batches are the stream either way, and the pace is no input', () => {
    const feeds = replayFeeds(transcript.raw)
    expect(feeds.filter((feed) => feed !== null).length).toBeGreaterThan(1)
    // Replayed again from the start, a later and an earlier clock change nothing about a batch's feed.
    vi.advanceTimersByTime(60_000)
    expect(json(replayFeeds(transcript.raw))).toBe(json(feeds))
  })

  it('puts the same cue on screen for every batch whether 0.4 s or 8 s passed since the last', () => {
    const dwell = replayCues(transcript.raw, 400)
    const deciding = replayCues(transcript.raw, 8000)
    expect(dwell.length).toBeGreaterThan(1)
    expect(json(deciding)).toBe(json(dwell))
  })

  it('never words another seat’s pass as automatic, quick or slow', () => {
    for (const feed of replayFeeds(transcript.raw)) {
      for (const line of feed?.lines ?? []) {
        expect(line.text).not.toMatch(/auto|quick|slow|second|deciding|thinking|waiting/i)
      }
      expect(feed?.announcement ?? '').not.toMatch(/auto|quick|slow|second|deciding|thinking|waiting/i)
    }
  })
})
