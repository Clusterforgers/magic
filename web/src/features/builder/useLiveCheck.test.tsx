// Goal: the live check asks for the draft's report once the draft has rested for the pause and no sooner, restarts the pause on every change, cancels the request in flight when the draft changes, shows the saved report until a check of the draft comes back and the last good report through a failure, returns to the saved report the moment the draft is the saved list again, treats a rename, new notes and a deck with no cards as nothing to check, ignores an answer for a list the draft has left, and asks again at once on a retry.
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { deckView, lineOf, reportCard, reportOf } from '@/features/decks/fakeDecks'
import { DeckError } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type { CheckRequest, DeckReport } from '@/protocol/generated'
import { addCard, draftOf } from './deckEdit'
import type { Draft } from './deckEdit'
import { BOLT, ELVES, GROWTH, MOUNTAIN } from './renderBuilder'
import { CHECK_PAUSE_MS, useLiveCheck } from './useLiveCheck'

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  vi.useRealTimers()
})

interface Pending {
  request: CheckRequest
  signal: AbortSignal | undefined
  resolve: (report: DeckReport) => void
  reject: (error: unknown) => void
}

/** A deck client whose checks the test answers by hand, so it decides what is in flight. */
function manual() {
  const asked: Pending[] = []
  const unused = () => Promise.reject(new Error('not used'))
  const api: DecksApi = {
    list: unused,
    get: unused,
    create: unused,
    update: unused,
    importList: unused,
    remove: unused,
    exportUrl: () => '',
    check: (request, signal) =>
      new Promise<DeckReport>((resolve, reject) => {
        asked.push({ request, signal, resolve, reject })
      }),
  }
  return { api, asked }
}

const SAVED = draftOf(
  deckView({
    format: 'modern',
    cards: [lineOf('Mountain', 20, { card: MOUNTAIN }), lineOf('Lightning Bolt', 4, { card: BOLT })],
  }),
)
const reportWith = (...names: Array<[string, number]>): DeckReport =>
  reportOf(
    names.map(([name, count]) => reportCard(name, { count })),
    [],
    { format: 'modern' },
  )
const SAVED_REPORT = reportWith(['Mountain', 20], ['Lightning Bolt', 4])
const WITH_GROWTH = addCard(SAVED, GROWTH, 'main')
const WITH_ELVES = addCard(WITH_GROWTH, ELVES, 'main')
const GROWTH_REPORT = reportWith(['Mountain', 20], ['Lightning Bolt', 4], ['Giant Growth', 1])
const ELVES_REPORT = reportWith(
  ['Mountain', 20],
  ['Lightning Bolt', 4],
  ['Giant Growth', 1],
  ['Llanowar Elves', 1],
)

function setup() {
  const { api, asked } = manual()
  const hook = renderHook(
    ({ draft }: { draft: Draft }) => useLiveCheck(api, { draft: SAVED, report: SAVED_REPORT }, draft),
    { initialProps: { draft: SAVED } },
  )
  const change = (draft: Draft) => hook.rerender({ draft })
  return { ...hook, change, asked }
}
const advance = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms))

describe('what needs no check', () => {
  it('sends nothing while the draft is the saved list, however it is renamed', async () => {
    const { result, change, asked } = setup()
    change({ ...SAVED, name: 'Another name', notes: 'Some notes' })
    await advance(CHECK_PAUSE_MS * 5)
    expect(asked).toHaveLength(0)
    expect(result.current).toMatchObject({ report: SAVED_REPORT, source: 'saved', status: 'current' })
  })

  it('sends nothing for a deck with no cards', async () => {
    const { result, change, asked } = setup()
    change({ ...SAVED, lines: [] })
    await advance(CHECK_PAUSE_MS * 5)
    expect(asked).toHaveLength(0)
    expect(result.current.status).toBe('current')
  })
})

describe('the pause', () => {
  it('waits for the pause, then asks for the list the draft holds', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    expect(result.current).toMatchObject({ report: SAVED_REPORT, source: 'saved', status: 'checking' })
    await advance(CHECK_PAUSE_MS - 1)
    expect(asked).toHaveLength(0)
    await advance(1)
    expect(asked).toHaveLength(1)
    expect(asked[0]?.request).toEqual({
      deck: null,
      format: 'modern',
      cards: [
        { zone: 'main', oracle_id: 'oracle-mountain', printing_id: null, count: 20 },
        { zone: 'main', oracle_id: 'oracle-bolt', printing_id: null, count: 4 },
        { zone: 'main', oracle_id: 'oracle-growth', printing_id: null, count: 1 },
      ],
    })
    expect(asked[0]?.signal?.aborted).toBe(false)
  })

  it('starts the pause again with every change', async () => {
    const { change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS - 1)
    change(WITH_ELVES)
    await advance(CHECK_PAUSE_MS - 1)
    expect(asked).toHaveLength(0)
    await advance(1)
    expect(asked).toHaveLength(1)
    expect(asked[0]?.request.cards).toHaveLength(4)
  })

  it('asks for a changed format too', async () => {
    const { change, asked } = setup()
    change({ ...SAVED, format: 'commander' })
    await advance(CHECK_PAUSE_MS)
    expect(asked[0]?.request.format).toBe('commander')
  })
})

describe('the report of the draft', () => {
  it('shows the saved report while the check is on its way and the draft report once it comes back', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    expect(result.current).toMatchObject({ report: SAVED_REPORT, source: 'saved', status: 'checking' })
    await act(async () => asked[0]?.resolve(GROWTH_REPORT))
    expect(result.current).toMatchObject({
      report: GROWTH_REPORT,
      source: 'draft',
      status: 'current',
      error: null,
    })
  })

  it('cancels the request in flight when the draft changes, and takes only the answer for the newest list', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    change(WITH_ELVES)
    expect(asked[0]?.signal?.aborted).toBe(true)
    await advance(CHECK_PAUSE_MS)
    expect(asked).toHaveLength(2)
    await act(async () => asked[1]?.resolve(ELVES_REPORT))
    await act(async () => asked[0]?.resolve(GROWTH_REPORT))
    expect(result.current).toMatchObject({ report: ELVES_REPORT, source: 'draft', status: 'current' })
  })

  it('keeps the last good report on screen while the next one is on its way', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    await act(async () => asked[0]?.resolve(GROWTH_REPORT))
    change(WITH_ELVES)
    expect(result.current).toMatchObject({ report: GROWTH_REPORT, source: 'draft', status: 'checking' })
  })
})

describe('a check that fails', () => {
  const down = () => new DeckError('network', 0, 'no route')

  it('keeps the saved report and says why', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    await act(async () => asked[0]?.reject(down()))
    expect(result.current).toMatchObject({ report: SAVED_REPORT, source: 'saved', status: 'failed' })
    expect(result.current.error).toMatch(/Could not reach the server/)
  })

  it('keeps the last good draft report through a later failure', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    await act(async () => asked[0]?.resolve(GROWTH_REPORT))
    change(WITH_ELVES)
    await advance(CHECK_PAUSE_MS)
    await act(async () => asked[1]?.reject(down()))
    expect(result.current).toMatchObject({ report: GROWTH_REPORT, source: 'draft', status: 'failed' })
  })

  it('asks again at once on a retry, and a good answer clears the failure', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    await act(async () => asked[0]?.reject(down()))
    act(() => result.current.retry())
    expect(asked).toHaveLength(2)
    expect(result.current.status).toBe('checking')
    await act(async () => asked[1]?.resolve(GROWTH_REPORT))
    expect(result.current).toMatchObject({ report: GROWTH_REPORT, status: 'current', error: null })
  })

  it('forgets a failure when the draft changes again', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    await act(async () => asked[0]?.reject(down()))
    change(WITH_ELVES)
    expect(result.current.status).toBe('checking')
  })
})

describe('back to the saved list', () => {
  it('shows the saved report at once, cancels what is in flight and waits a fresh pause for the next change', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    change(SAVED)
    expect(asked[0]?.signal?.aborted).toBe(true)
    expect(result.current).toMatchObject({ report: SAVED_REPORT, source: 'saved', status: 'current' })
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS - 1)
    expect(asked).toHaveLength(1)
    await advance(1)
    expect(asked).toHaveLength(2)
  })

  it('drops the draft report, so a different change starts from the saved one', async () => {
    const { result, change, asked } = setup()
    change(WITH_GROWTH)
    await advance(CHECK_PAUSE_MS)
    await act(async () => asked[0]?.resolve(GROWTH_REPORT))
    change(SAVED)
    change(addCard(SAVED, ELVES, 'main'))
    expect(result.current).toMatchObject({ report: SAVED_REPORT, source: 'saved', status: 'checking' })
  })
})
