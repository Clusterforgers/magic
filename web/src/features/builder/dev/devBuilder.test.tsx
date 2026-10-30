// Goal: /dev/builder reads the deck from the address (shrugging off nonsense), serves the real builder over a fixture deck with a card of each support state, a card the game cannot host and illegal lines, whose report agrees with its lines, gives every card a detail whose clauses lie in its text and match the report, answers the check of a list the way the saved report reads, opens a preview with the underlined clause and follows the draft with the live check, and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { scaled } from '@/test-utils/loadFactor'
import { reportFor } from '../fakeCheck'
import { parseDevBuilderSearch } from './builderSearch'
import {
  BUILDER_FIXTURE_MARKER,
  DEV_DECKS,
  fixtureCards,
  fixtureCheck,
  fixtureDetails,
  fixtureView,
} from './fixture'

/** The real route tree, loaded fresh so the route sees the `import.meta.env.DEV` the test has set. */
async function visit(url: string) {
  vi.resetModules()
  const { routeTree } = await import('@/routeTree.gen')
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  render(<RouterProvider router={router} />)
  await act(async () => {
    await router.load()
  })
  return router
}

afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
})

describe('parseDevBuilderSearch', () => {
  it.each([
    [{}, 'mixed'],
    [{ deck: 'verified' }, 'verified'],
    [{ deck: 'changed' }, 'changed'],
    [{ deck: 'nonsense' }, 'mixed'],
    [{ deck: 3 }, 'mixed'],
    [{ deck: null }, 'mixed'],
  ])('reads %j as the %s deck', (search, deck) => {
    expect(parseDevBuilderSearch(search).deck).toBe(deck)
  })
})

describe('the fixture decks', () => {
  it('has a card of each support state in the mixed deck, every flagged one with a reason', () => {
    const { report } = fixtureView('mixed')
    expect(new Set(report.cards.map((card) => card.support))).toEqual(
      new Set(['verified', 'partial', 'manual', 'stale', 'unplayable']),
    )
    for (const card of report.cards.filter((entry) => entry.support !== 'verified')) {
      expect(card.reasons.length, card.name).toBeGreaterThan(0)
    }
    expect(report.cards.some((card) => card.reasons.some((entry) => entry.face > 0))).toBe(true)
  })

  it('has illegal lines in the mixed deck, which make it blocked', () => {
    const { report, state } = fixtureView('mixed')
    expect(report.legality.map((issue) => issue.code)).toEqual(['deck_size', 'copies', 'banned'])
    expect(state).toBe('blocked')
  })

  it.each(DEV_DECKS)('has a report that agrees with the lines of the %s deck', (deck) => {
    const view = fixtureView(deck)
    const lines = view.cards.map(
      (line) => `${line.zone}:${line.card.oracle_id}:${line.count}:${line.card.support}`,
    )
    const reported = view.report.cards.map(
      (card) => `${card.zone}:${card.oracle_id}:${card.count}:${card.support}`,
    )
    expect(reported).toEqual(lines)
    expect(view.report.summary.counts.total).toBe(view.cards.reduce((sum, line) => sum + line.count, 0))
  })

  it.each([
    ['verified', 'verified', false, 0],
    ['manual', 'manual_play', false, 0],
    ['mixed', 'blocked', false, 3],
    ['changed', 'unchecked', true, 3],
    ['unchecked', 'unchecked', false, 3],
  ] as const)('the %s deck is %s, changed=%s, with %i legality issues', (deck, state, changed, issues) => {
    const view = fixtureView(deck)
    expect([view.state, view.changed, view.report.legality.length]).toEqual([state, changed, issues])
    expect(view.verified).toBe(state === 'verified')
  })

  it('holds a legal deck of exactly 100 cards in the verified and manual decks', () => {
    for (const deck of ['verified', 'manual'] as const) {
      const view = fixtureView(deck)
      expect(
        view.cards.filter((line) => line.zone !== 'sideboard').reduce((n, line) => n + line.count, 0),
      ).toBe(100)
    }
  })

  it('offers the cards of the fixture, and the longest card name, to the search', () => {
    const names = fixtureCards().map((card) => card.name)
    expect(names).toContain('Cogwork Librarian')
    expect(Math.max(...names.map((name) => name.length))).toBeGreaterThan(120)
  })
})

describe('the fixture details and the fake check', () => {
  const details = fixtureDetails()

  it('has a detail for every card of the search', () => {
    expect(details.map((detail) => detail.card)).toEqual(fixtureCards())
    expect(details.every((detail) => detail.faces.length > 0)).toBe(true)
  })

  it('keeps every clause of a detail inside the text of its face, over words and not over blanks', () => {
    for (const detail of details) {
      for (const clause of detail.reasons) {
        const text = Array.from(detail.faces[clause.face]?.oracle_text ?? '')
        expect(clause.span.end, detail.card.name).toBeLessThanOrEqual(text.length)
        expect(clause.span.start, detail.card.name).toBeLessThan(clause.span.end)
        expect(text.slice(clause.span.start, clause.span.end).join('').trim(), detail.card.name).not.toBe('')
      }
    }
  })

  it('has a detail with clauses exactly for the cards the report flags, and the same clauses', () => {
    const { report } = fixtureView('mixed')
    for (const flagged of report.cards) {
      const detail = details.find((entry) => entry.card.oracle_id === flagged.oracle_id)
      expect(detail?.reasons, flagged.name).toEqual(flagged.reasons)
      expect(detail?.card.support, flagged.name).toBe(flagged.support)
    }
  })

  it('has a clause on the back face and clauses that cover part of the text and the whole of it', () => {
    const reasons = details.flatMap((detail) =>
      detail.reasons.map((clause) => ({
        face: clause.face,
        whole:
          clause.span.start === 0 &&
          clause.span.end === Array.from(detail.faces[clause.face]?.oracle_text ?? '').length,
      })),
    )
    expect(reasons.some((entry) => entry.face > 0)).toBe(true)
    expect(reasons.some((entry) => entry.whole)).toBe(true)
    expect(reasons.some((entry) => !entry.whole)).toBe(true)
  })

  it.each(DEV_DECKS)('answers the check of the %s deck as it reads in the saved report', (deck) => {
    const view = fixtureView(deck)
    const asked = reportFor(
      {
        deck: null,
        format: view.format,
        cards: view.cards.map((line) => ({
          zone: line.zone,
          oracle_id: line.card.oracle_id,
          printing_id: null,
          count: line.count,
        })),
      },
      fixtureCheck(),
    )
    const byLine = (cards: typeof asked.cards) =>
      [...cards].sort((a, b) => `${a.zone}:${a.oracle_id}`.localeCompare(`${b.zone}:${b.oracle_id}`))
    expect(byLine(asked.cards)).toEqual(byLine(view.report.cards))
    expect(asked.legality).toEqual(view.report.legality)
    expect(asked.summary).toEqual(view.report.summary)
  })
})

describe('/dev/builder in development', () => {
  it('shows the mixed deck: a mark of each state, the legality section and the fixture marker', async () => {
    await visit('/dev/builder')
    const deck = await screen.findByRole('region', { name: 'Deck' })
    for (const state of ['partial', 'manual', 'stale', 'unplayable']) {
      expect(deck.querySelector(`li[data-line] [data-support="${state}"]`), state).not.toBeNull()
    }
    expect(within(deck).getByRole('region', { name: /^Not legal in Commander/ })).toBeInTheDocument()
    expect(within(deck).getByText('Blocked')).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${BUILDER_FIXTURE_MARKER}"]`)).not.toBeNull()
  }, 10_000)

  it('opens the preview of a card from its name, with the clause of its first reason underlined', async () => {
    await visit('/dev/builder')
    const deck = await screen.findByRole('region', { name: 'Deck' })
    await userEvent.setup().click(within(deck).getByRole('button', { name: 'Goblin Rabblemaster' }))
    const preview = await screen.findByRole('dialog', { name: 'Goblin Rabblemaster' })
    await waitFor(() => expect(preview.querySelector('[data-clause="1"]')).not.toBeNull())
    expect(preview.querySelector('[data-clause="1"]')).toHaveTextContent(
      'Other Goblin creatures you control attack each combat if able.',
    )
    expect(within(preview).getByText('The engine does not run attack requirements yet.')).toBeInTheDocument()
  }, 10_000)

  it('follows the draft with the live check: another copy of a flagged card changes the counts', async () => {
    await visit('/dev/builder?deck=manual')
    const deck = await screen.findByRole('region', { name: 'Deck' })
    const counts = () => deck.querySelector('[data-slot="check-counts"]')
    expect(counts()).toHaveTextContent(/^98 of 103 verified, 5 played by hand$/)
    expect(within(deck).queryByRole('region', { name: /^Not legal/ })).toBeNull()
    await userEvent.setup().click(within(deck).getByRole('button', { name: 'Add one Chaos Orb' }))
    expect(counts()).toHaveTextContent(/ · as saved$/)
    await waitFor(() => expect(counts()).toHaveTextContent(/^98 of 104 verified, 6 played by hand$/), {
      timeout: scaled(5000),
    })
    expect(within(deck).getByRole('region', { name: /^Not legal in Commander/ })).toHaveTextContent(
      '2 issues',
    )
  }, 15_000)

  it.each([
    ['?deck=verified', 'verified'],
    ['?deck=manual', 'manual_play'],
    ['?deck=changed', 'changed'],
    ['?deck=unchecked', 'unchecked'],
    ['?deck=nonsense', 'blocked'],
  ])('reads %s', async (search, marker) => {
    await visit(`/dev/builder${search}`)
    const deck = await screen.findByRole('region', { name: 'Deck' })
    expect(deck.querySelector('[data-marker]')).toHaveAttribute('data-marker', marker)
  })
})

describe('/dev/builder in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/builder')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
    expect(screen.queryByRole('region', { name: 'Deck' })).toBeNull()
  })
})
