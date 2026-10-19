// Goal: the deck pane shows the deck check as the report gives it: a counts line from the summary, every line whose card is not verified with its mark, edge and reasons (a verified line with none), the Issues only switch and the sort that puts issues first, legality in a section of its own that holds no support mark, and beside the name the marker, which says the badge's word for each saved state and "Changed since the last check" once the draft differs or the stored check is stale.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { legalityIssue, lineOf, reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { CardHit, DeckLineView, DeckView, Reason, Support } from '@/protocol/generated'
import {
  absent,
  BOLT,
  deckLine,
  deckPane,
  DURESS,
  ELVES,
  GROWTH,
  MOUNTAIN,
  renderBuilder,
  SOL_RING,
} from './renderBuilder'

const pane = () => within(deckPane())
const reason = (text: string, face = 0): Reason => ({ face, span: { start: 0, end: 10 }, text })

interface Entry {
  card: CardHit
  count: number
  support: Support
  zone?: 'main' | 'sideboard'
  reasons?: Reason[]
}

const ENTRIES: Entry[] = [
  { card: MOUNTAIN, count: 20, support: 'verified' },
  { card: BOLT, count: 4, support: 'partial', reasons: [reason('the engine does not run X yet')] },
  { card: GROWTH, count: 2, support: 'manual', reasons: [reason('no rules for this card yet')] },
  {
    card: SOL_RING,
    count: 1,
    support: 'stale',
    reasons: [reason('the oracle text changed since the rules were written')],
  },
  {
    card: ELVES,
    count: 1,
    support: 'unplayable',
    reasons: [reason('the card cannot be hosted', 0), reason('its back is a puzzle', 1)],
  },
  { card: DURESS, count: 2, support: 'verified', zone: 'sideboard' },
]

/** A deck of `entries` whose report is the one the server would work out, with `over` on the view. */
function checked(over: Partial<DeckView> = {}, entries: Entry[] = ENTRIES, legality = legalityIssues([])) {
  const cards: DeckLineView[] = entries.map((entry) =>
    lineOf(entry.card.name, entry.count, {
      zone: entry.zone ?? 'main',
      card: { ...entry.card, support: entry.support },
    }),
  )
  const report = reportOf(
    entries.map((entry) =>
      reportCard(entry.card.name, {
        oracle_id: entry.card.oracle_id,
        zone: entry.zone ?? 'main',
        count: entry.count,
        support: entry.support,
        reasons: entry.reasons ?? [],
      }),
    ),
    legality,
    { format: 'modern' },
  )
  return { cards, report, state: report.summary.state, verified: false, ...over }
}

function legalityIssues(texts: string[]) {
  return texts.map((text) => legalityIssue(text))
}

const mainOrder = (): Array<string | null | undefined> =>
  [...deckPane().querySelectorAll('li[data-line^="main:"] > [data-slot="card-name"]')].map(
    (name) => name.textContent,
  )

describe('the counts of the check', () => {
  it('says how many copies the table plays by itself, from the summary of the report', async () => {
    await renderBuilder({ view: checked() })
    expect(deckPane().querySelector('[data-slot="check-counts"]')).toHaveTextContent(
      /^22 of 30 verified, 2 manual, 4 partial, 1 stale, 1 unplayable$/,
    )
  })

  it('says only "N of N verified" for a deck with nothing to flag', async () => {
    await renderBuilder({ view: checked({}, [{ card: MOUNTAIN, count: 24, support: 'verified' }]) })
    expect(deckPane().querySelector('[data-slot="check-counts"]')).toHaveTextContent(/^24 of 24 verified$/)
  })
})

describe('the mark of a line', () => {
  it.each([
    ['Lightning Bolt', 'Partial', 'The engine does not run X yet.'],
    ['Giant Growth', 'Manual', 'No rules for this card yet.'],
    ['Sol Ring', 'Stale', 'The oracle text changed since the rules were written.'],
    ['Llanowar Elves', 'Unplayable', 'Front face: the card cannot be hosted.'],
  ])('marks %s as %s with the reason the report gives, as a plain line', async (name, word, text) => {
    await renderBuilder({ view: checked() })
    const row = deckLine(name)
    const state = word.toLowerCase()
    expect(row).toHaveAttribute('data-support', state)
    expect(row.className).toContain(`border-support-${state}`)
    expect(within(row).getByText(word)).toBeInTheDocument()
    expect(within(row).getByText(text)).toBeInTheDocument()
    expect(within(row).getByRole('list', { name: `Why ${name} is not verified` })).toBeInTheDocument()
  })

  it('names the face of a clause that is not on the front, on every line of that card', async () => {
    await renderBuilder({ view: checked() })
    const lines = within(deckLine('Llanowar Elves')).getAllByRole('listitem')
    expect(lines.map((line) => line.textContent)).toEqual([
      'Front face: the card cannot be hosted.',
      'Back face: its back is a puzzle.',
    ])
  })

  it.each(['Mountain', 'Duress'])('marks nothing on %s, whose card is verified', async (name) => {
    await renderBuilder({ view: checked() })
    const row = deckLine(name)
    expect(row).toHaveAttribute('data-support', 'verified')
    expect(row.querySelector('[data-slot="line-check"]')).toBeNull()
    expect(row.className).toContain('border-transparent')
  })

  it('marks a card that is not in the report by the state it came with', async () => {
    const view = checked()
    await renderBuilder({ view: { ...view, report: { ...view.report, cards: [] } } })
    expect(within(deckLine('Giant Growth')).getByText('Manual')).toBeInTheDocument()
    expect(within(deckLine('Giant Growth')).queryByRole('list')).toBeNull()
  })
})

describe('Issues only and the sort', () => {
  it('hides the verified lines, and a zone with nothing to show, until the switch is turned off', async () => {
    const { user } = await renderBuilder({ view: checked() })
    const only = pane().getByRole('switch', { name: 'Issues only' })
    expect(only).toHaveAttribute('aria-checked', 'false')
    await user.click(only)
    expect(only).toHaveAttribute('aria-checked', 'true')
    expect(absent('Mountain')).toBe(true)
    expect(deckLine('Giant Growth')).toBeInTheDocument()
    expect(pane().queryByRole('heading', { name: /^Sideboard/ })).toBeNull()
    await user.click(only)
    expect(deckLine('Mountain')).toBeInTheDocument()
    expect(pane().getByRole('heading', { name: /^Sideboard/ })).toBeInTheDocument()
  })

  it('says so when no card has an issue', async () => {
    const { user } = await renderBuilder({
      view: checked({}, [{ card: MOUNTAIN, count: 24, support: 'verified' }]),
    })
    await user.click(pane().getByRole('switch', { name: 'Issues only' }))
    expect(pane().getByRole('heading', { name: 'No cards with issues' })).toBeInTheDocument()
    expect(absent('Mountain')).toBe(true)
  })

  it('orders the lines by name, or with the worst state first', async () => {
    const { user } = await renderBuilder({ view: checked() })
    expect(mainOrder()).toEqual(['Giant Growth', 'Lightning Bolt', 'Llanowar Elves', 'Mountain', 'Sol Ring'])
    await user.selectOptions(pane().getByRole('combobox', { name: 'Sort cards' }), 'issues')
    expect(mainOrder()).toEqual(['Llanowar Elves', 'Giant Growth', 'Sol Ring', 'Lightning Bolt', 'Mountain'])
    await user.selectOptions(pane().getByRole('combobox', { name: 'Sort cards' }), 'name')
    expect(mainOrder()).toEqual(['Giant Growth', 'Lightning Bolt', 'Llanowar Elves', 'Mountain', 'Sol Ring'])
  })
})

describe('the legality section', () => {
  const LEGALITY = [
    'the main deck needs at least 60 cards, this one has 30',
    'Black Lotus is banned in modern',
  ]

  it('lists the issues as plain sentences, apart from the support marks of the lines', async () => {
    await renderBuilder({ view: checked({}, ENTRIES, legalityIssues(LEGALITY)) })
    const section = pane().getByRole('region', { name: /^Not legal in Modern/ })
    expect(section).toHaveTextContent('2 issues')
    expect(
      within(section)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['The main deck needs at least 60 cards, this one has 30.', 'Black Lotus is banned in modern.'])
    expect(section.querySelector('[data-support]')).toBeNull()
    expect(deckPane().querySelectorAll('li[data-line] [data-slot="legality"]')).toHaveLength(0)
    expect(section.closest('li[data-line]')).toBeNull()
  })

  it('stays when only the issues are shown, and does not count in the support counts', async () => {
    const { user } = await renderBuilder({ view: checked({}, ENTRIES, legalityIssues(LEGALITY)) })
    await user.click(pane().getByRole('switch', { name: 'Issues only' }))
    expect(pane().getByRole('region', { name: /^Not legal in Modern/ })).toBeInTheDocument()
    expect(deckPane().querySelector('[data-slot="check-counts"]')).toHaveTextContent(/^22 of 30 verified/)
  })

  it('is not drawn for a deck that is legal', async () => {
    await renderBuilder({ view: checked() })
    expect(pane().queryByRole('region', { name: /^Not legal/ })).toBeNull()
  })
})

describe('the marker beside the name', () => {
  it.each([
    ['a clean saved check', { state: 'verified', verified: true }, 'Verified'],
    ['a deck played by hand', { state: 'manual_play' }, 'Manual play'],
    ['a blocked deck', { state: 'blocked' }, 'Blocked'],
    ['a deck never checked', { state: 'unchecked', changed: false }, 'Unchecked'],
    ['a stale stored check', { state: 'unchecked', changed: true }, 'Changed since the last check'],
  ] as const)('says the word for %s', async (_what, over, word) => {
    await renderBuilder({ view: over })
    const marker = deckPane().querySelector('[data-marker]')
    expect(marker).toHaveTextContent(new RegExp(`^${word}$`))
    expect(pane().queryByText('Unsaved changes')).toBeNull()
  })

  it('turns to "Changed since the last check" the moment the draft differs, then follows the save', async () => {
    const { user } = await renderBuilder({ view: { state: 'verified', verified: true } })
    expect(deckPane().querySelector('[data-marker]')).toHaveAttribute('data-marker', 'verified')
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    expect(deckPane().querySelector('[data-marker]')).toHaveTextContent(/^Changed since the last check$/)
    expect(pane().getByText('Unsaved changes')).toBeInTheDocument()
    expect(deckPane().querySelector('[data-slot="check-counts"]')).toHaveTextContent(/ · as saved$/)
    await user.click(pane().getByRole('button', { name: 'Save' }))
    expect(await pane().findByText('Verified')).toBeInTheDocument()
    expect(deckPane().querySelector('[data-marker]')).toHaveAttribute('data-marker', 'verified')
    expect(deckPane().querySelector('[data-slot="check-counts"]')).not.toHaveTextContent('as saved')
  })
})
