// Goal: while cards are added to or taken out of the draft the builder checks it live: the counts line, the marks, the reasons and the legality section follow the draft after the pause, "Checking…" shows while the counts are old and "as saved" says so until a check of the draft has come back, a failed check keeps the last report with a note and a Try again, a rename asks nothing, and taking the draft back to the saved list shows the saved report at once.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { flushFakeClock, installFakeClock, settle } from '@/test-utils/fakeClock'
import { scaled } from '@/test-utils/loadFactor'
import { deckError, legalityIssue, reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { CheckRequest } from '@/protocol/generated'
import type { CheckScript } from './fakeCheck'
import {
  deckLine,
  deckPane,
  GROWTH,
  POOL,
  renderBuilder,
  searchBox,
  searchPane,
  SOL_RING,
} from './renderBuilder'
import type { BuilderScript } from './renderBuilder'
import { CHECK_PAUSE_MS } from './useLiveCheck'

const SAVED_REPORT = reportOf(
  [
    reportCard('Mountain', { oracle_id: 'oracle-mountain', count: 20 }),
    reportCard('Lightning Bolt', { oracle_id: 'oracle-bolt', count: 4 }),
    reportCard('Duress', { oracle_id: 'oracle-duress', zone: 'sideboard', count: 2 }),
  ],
  [],
  { format: 'modern' },
)

/** The fake check judges Giant Growth as played by hand, and Giant Growth and Sol Ring as banned. */
const judging = (extra: Partial<CheckScript> = {}): BuilderScript => ({
  view: { state: 'verified', verified: true, report: SAVED_REPORT },
  check: {
    cards: POOL.map((card) => (card.oracle_id === GROWTH.oracle_id ? { ...card, support: 'manual' } : card)),
    reasons: {
      [GROWTH.oracle_id]: [{ face: 0, span: { start: 0, end: 10 }, text: 'no rules for this card yet' }],
    },
    legality: (request: CheckRequest) =>
      [GROWTH, SOL_RING]
        .filter((card) => request.cards?.some((line) => line.oracle_id === card.oracle_id))
        .map((card) =>
          legalityIssue(`${card.name} is banned in modern`, { code: 'banned', name: card.name }),
        ),
    ...extra,
  },
})

const counts = () => deckPane().querySelector('[data-slot="check-counts"]')
const status = () => deckPane().querySelector('[data-slot="check-status"]')
const SAVED_COUNTS = '26 of 26 verified'
const WAIT = { timeout: scaled(4000) }

async function add(user: Awaited<ReturnType<typeof renderBuilder>>['user'], query: string, name: string) {
  const box = searchBox()
  await user.clear(box)
  await user.type(box, query)
  await user.click(await within(searchPane()).findByRole('button', { name: `Add ${name} to main deck` }))
}
// A fake clock: the check waits out its pause only when a test lets the time pass.
beforeEach(installFakeClock)
afterEach(flushFakeClock)

describe('the live check in the builder', () => {
  it('follows the draft: the counts, the mark and the reason of a card come after the pause', async () => {
    const { user, checks } = await renderBuilder(judging())
    expect(counts()).toHaveTextContent(new RegExp(`^${SAVED_COUNTS}$`))
    await add(user, 'giant', 'Giant Growth')
    expect(counts()).toHaveTextContent(new RegExp(`^${SAVED_COUNTS} · as saved$`))
    expect(status()).toHaveTextContent('Checking…')
    expect(checks.requests).toHaveLength(0)

    await waitFor(() => expect(counts()).toHaveTextContent(/^26 of 27 verified, 1 played by hand$/), WAIT)
    expect(counts()).not.toHaveTextContent('as saved')
    expect(status()).not.toHaveTextContent('Checking')
    expect(status()).toHaveTextContent('Checked: 26 of 27 verified, 1 played by hand.')
    const line = deckLine('Giant Growth')
    expect(line).toHaveAttribute('data-support', 'manual')
    expect(within(line).getByText('No rules for this card yet.')).toBeInTheDocument()
    expect(checks.requests).toHaveLength(1)
    expect(checks.requests[0]?.cards?.map((card) => card.oracle_id)).toContain('oracle-growth')
    expect(checks.requests[0]?.deck).toBeNull()
    expect(checks.requests[0]?.format).toBe('modern')
  })

  it('counts the issues of the legality section again as cards are added', async () => {
    const { user } = await renderBuilder(judging())
    expect(within(deckPane()).queryByRole('region', { name: /^Not legal/ })).toBeNull()
    await add(user, 'giant', 'Giant Growth')
    const one = await within(deckPane()).findByRole('region', { name: /^Not legal in Modern/ }, WAIT)
    expect(one).toHaveTextContent('1 issue')
    expect(one).not.toHaveTextContent('as saved')
    await add(user, 'sol', 'Sol Ring')
    await waitFor(
      () =>
        expect(within(deckPane()).getByRole('region', { name: /^Not legal in Modern/ })).toHaveTextContent(
          '2 issues',
        ),
      WAIT,
    )
  })

  it('shows the saved counts at once when the draft is the saved list again, and asks nothing more', async () => {
    const { user, checks } = await renderBuilder(judging())
    await add(user, 'giant', 'Giant Growth')
    await waitFor(() => expect(counts()).toHaveTextContent(/^26 of 27 verified, 1 played by hand$/), WAIT)
    await user.click(screen.getByRole('button', { name: 'Remove one Giant Growth' }))
    expect(counts()).toHaveTextContent(new RegExp(`^${SAVED_COUNTS}$`))
    expect(within(deckPane()).queryByRole('region', { name: /^Not legal/ })).toBeNull()
    await settle(CHECK_PAUSE_MS * 2)
    expect(checks.requests).toHaveLength(1)
  })

  it('asks nothing when only the name is edited', async () => {
    const { user, checks } = await renderBuilder(judging())
    await user.type(within(deckPane()).getByRole('textbox', { name: 'Deck name' }), ' II')
    await settle(CHECK_PAUSE_MS * 2)
    expect(checks.requests).toHaveLength(0)
    expect(counts()).toHaveTextContent(new RegExp(`^${SAVED_COUNTS}$`))
  })

  it('cancels the request in flight when the draft changes again', async () => {
    const waiting = new Promise<Response>(() => undefined)
    const { user, checks } = await renderBuilder(judging({ answer: () => waiting }))
    await add(user, 'giant', 'Giant Growth')
    await waitFor(() => expect(checks.requests).toHaveLength(1), WAIT)
    expect(checks.signals[0]?.aborted).toBe(false)
    await user.click(screen.getByRole('button', { name: 'Add one Giant Growth' }))
    await waitFor(() => expect(checks.signals[0]?.aborted).toBe(true))
    expect(status()).toHaveTextContent('Checking…')
  })
})

describe('a check that fails', () => {
  it('keeps the saved counts with a note and Try again, which recovers', async () => {
    let down = true
    const { user, checks } = await renderBuilder(
      judging({ answer: () => (down ? deckError(500, 'internal', 'boom') : undefined) }),
    )
    await add(user, 'giant', 'Giant Growth')
    const note = await waitFor(() => {
      const found = deckPane().querySelector('[data-slot="check-failed"]')
      expect(found).not.toBeNull()
      return found as HTMLElement
    }, WAIT)
    expect(note).toHaveTextContent('Could not check the changes. The counts are those of the saved deck.')
    expect(note).toHaveTextContent('The server ran into a problem.')
    expect(counts()).toHaveTextContent(new RegExp(`^${SAVED_COUNTS} · as saved$`))
    expect(status()).not.toHaveTextContent('Checking')

    down = false
    await user.click(within(note).getByRole('button', { name: 'Try again' }))
    await waitFor(() => expect(counts()).toHaveTextContent(/^26 of 27 verified, 1 played by hand$/), WAIT)
    expect(deckPane().querySelector('[data-slot="check-failed"]')).toBeNull()
    expect(checks.requests).toHaveLength(2)
  })
})
