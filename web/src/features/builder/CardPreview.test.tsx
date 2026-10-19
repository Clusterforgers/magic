// Goal: a card's name in the deck or the search opens its preview, which shows each face with its name, cost, type line, power and toughness and oracle text and the support mark, underlines every clause the game cannot resolve with a dotted line and a note number whose reason is spelled out under the text ("Back face" on a clause of another face), says when a card is fully supported, closes on Escape and on Close with focus back on the name that opened it, sits as a sheet at phone width, and designs its loading, failed (with Try again), unavailable and unknown-card states.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { cardHit, deckError, lineOf } from '@/features/decks/fakeDecks'
import type { CardDetail, CardFaceView, CardHit, Reason, Span } from '@/protocol/generated'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { cardDetail } from './fakeCards'
import { BOLT, copies, deckLine, MOUNTAIN, renderBuilder, searchBox, searchPane } from './renderBuilder'

const BOLT_TEXT = 'Lightning Bolt deals 3 damage to any target.'

const face = (name: string, over: Partial<CardFaceView> = {}): CardFaceView => ({
  name,
  mana_cost: '{R}',
  type_line: 'Instant',
  oracle_text: '',
  power: null,
  toughness: null,
  loyalty: null,
  defense: null,
  colors: ['R'],
  ...over,
})

/** The span of `phrase` in `text`, counted in characters as the server does. */
const spanOf = (text: string, phrase: string): Span => {
  const start = Array.from(text.slice(0, text.indexOf(phrase))).length
  return { start, end: start + Array.from(phrase).length }
}
const clause = (text: string, phrase: string, why: string, faceIndex = 0): Reason => ({
  face: faceIndex,
  span: spanOf(text, phrase),
  text: why,
})

const boltDetail = (over: Partial<CardDetail> = {}): CardDetail =>
  cardDetail('Lightning Bolt', {
    card: { ...BOLT },
    oracle_text: BOLT_TEXT,
    faces: [face('Lightning Bolt', { oracle_text: BOLT_TEXT })],
    ...over,
  })

const GIANT_TEXT =
  "Whenever Bonecrusher Giant becomes the target of a spell, Bonecrusher Giant deals 2 damage to that spell's controller."
const STOMP_TEXT =
  "Damage can't be prevented this turn. Stomp deals 2 damage to any target.\nThen exile this card."
const GIANT: CardHit = cardHit('Bonecrusher Giant // Stomp', {
  oracle_id: 'oracle-bonecrusher',
  mana_cost: '{2}{R} // {1}{R}',
  type_line: 'Creature — Giant // Instant — Adventure',
  support: 'partial',
})
const giantDetail = (): CardDetail =>
  cardDetail('Bonecrusher Giant // Stomp', {
    card: GIANT,
    oracle_text: `${GIANT_TEXT}\n//\n${STOMP_TEXT}`,
    faces: [
      face('Bonecrusher Giant', {
        mana_cost: '{2}{R}',
        type_line: 'Creature — Giant',
        oracle_text: GIANT_TEXT,
        power: '4',
        toughness: '3',
      }),
      face('Stomp', { mana_cost: '{1}{R}', type_line: 'Instant — Adventure', oracle_text: STOMP_TEXT }),
    ],
    reasons: [
      clause(
        STOMP_TEXT,
        "Damage can't be prevented this turn.",
        'the engine does not run damage prevention yet',
        1,
      ),
    ],
  })

const dialog = (name: string) => screen.findByRole('dialog', { name })
const oracle = (root: HTMLElement) => root.querySelector('[data-slot="oracle-text"]')
const underlined = (root: HTMLElement) =>
  [...root.querySelectorAll('[data-clause]')].map((span) => [
    span.textContent,
    span.getAttribute('data-clause'),
  ])
const nameButton = (line: HTMLElement, name: string) => within(line).getByRole('button', { name })

describe('opening the preview', () => {
  it('opens from the name of a deck line with the card, its face and the support mark', async () => {
    const { user } = await renderBuilder({
      cards: { details: [boltDetail({ card: { ...BOLT, support: 'partial' } })] },
    })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    expect(within(preview).getByText('Instant')).toBeInTheDocument()
    await waitFor(() => expect(oracle(preview)).toHaveTextContent(BOLT_TEXT))
    expect(preview.querySelector('[data-support="partial"]')).toHaveTextContent('Partial')
    expect(preview.querySelector('[data-slot="mana-cost"]')).not.toBeNull()
  })

  it('opens from the name of a search result, and the rest of the row still adds', async () => {
    const { user } = await renderBuilder({ cards: { details: [boltDetail()] } })
    await user.type(searchBox(), 'bolt')
    const row = await within(searchPane()).findByRole('button', { name: 'Lightning Bolt' })
    await user.click(row)
    const preview = await dialog('Lightning Bolt')
    await waitFor(() => expect(oracle(preview)).toHaveTextContent(BOLT_TEXT))
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(copies('Lightning Bolt')).toBe(4)
    await user.click(within(searchPane()).getByRole('button', { name: 'Add Lightning Bolt to main deck' }))
    expect(copies('Lightning Bolt')).toBe(5)
  })

  it('shows each face of a card with its name, cost, type line, power and toughness and text', async () => {
    const { user } = await renderBuilder({
      view: { cards: [lineOf('Mountain', 20, { card: MOUNTAIN }), lineOf('x', 1, { card: GIANT })] },
      cards: { details: [giantDetail()] },
    })
    await user.click(nameButton(deckLine('Bonecrusher Giant'), 'Bonecrusher Giant // Stomp'))
    const preview = await dialog('Bonecrusher Giant // Stomp')
    await waitFor(() => expect(preview.querySelectorAll('section[data-face]')).toHaveLength(2))
    const [front, back] = [...preview.querySelectorAll<HTMLElement>('section[data-face]')]
    expect(front).toHaveTextContent('Bonecrusher Giant')
    expect(front).toHaveTextContent('Creature — Giant')
    expect(front).toHaveTextContent('4/3')
    expect(front).toHaveTextContent('Power 4, toughness 3')
    expect(front?.querySelector('[data-slot="mana-cost"]')).not.toBeNull()
    expect(oracle(front as HTMLElement)).toHaveTextContent(GIANT_TEXT)
    expect(back).toHaveTextContent('Stomp')
    expect(back).toHaveTextContent('Instant — Adventure')
    expect(oracle(back as HTMLElement)).toHaveTextContent(/Then exile this card\./)
  })
})

describe('the clauses the game cannot resolve', () => {
  it('underlines the words of a reason with a dotted line and a note number, and spells the reason out under the text', async () => {
    const reasons = [clause(BOLT_TEXT, 'deals 3 damage', 'the engine does not run X yet')]
    const { user } = await renderBuilder({
      cards: { details: [boltDetail({ card: { ...BOLT, support: 'partial' }, reasons })] },
    })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    await waitFor(() => expect(underlined(preview)).toEqual([['deals 3 damage', '1']]))
    const mark = preview.querySelector('[data-clause]')
    expect(mark).toHaveClass('underline', 'decoration-dotted', 'decoration-support-partial')
    expect(oracle(preview)).toHaveTextContent('Lightning Bolt deals 3 damageNote 1 to any target.')
    expect(preview.querySelector('sup[data-note="1"]')).toHaveTextContent('Note 1')
    const list = within(preview).getByRole('list')
    expect(
      within(list)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['1The engine does not run X yet.'])
    expect(within(preview).queryByText(/Fully supported/)).toBeNull()
  })

  it('numbers several reasons and lets overlapping words carry both numbers', async () => {
    const reasons = [
      clause(BOLT_TEXT, 'Lightning Bolt deals 3', 'first'),
      clause(BOLT_TEXT, 'deals 3 damage', 'second'),
    ]
    const { user } = await renderBuilder({
      cards: { details: [boltDetail({ card: { ...BOLT, support: 'manual' }, reasons })] },
    })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    await waitFor(() =>
      expect(underlined(preview)).toEqual([
        ['Lightning Bolt', '1'],
        ['deals 3', '1 2'],
        ['damage', '2'],
      ]),
    )
    expect([...preview.querySelectorAll('sup')].map((note) => note.getAttribute('data-note'))).toEqual([
      '1',
      '2',
    ])
    expect(
      within(within(preview).getByRole('list'))
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['1First.', '2Second.'])
  })

  it('underlines a clause of the back face in the back text only and labels it "Back face"', async () => {
    const { user } = await renderBuilder({
      view: { cards: [lineOf('Mountain', 20, { card: MOUNTAIN }), lineOf('x', 1, { card: GIANT })] },
      cards: { details: [giantDetail()] },
    })
    await user.click(nameButton(deckLine('Bonecrusher Giant'), 'Bonecrusher Giant // Stomp'))
    const preview = await dialog('Bonecrusher Giant // Stomp')
    await waitFor(() => expect(preview.querySelectorAll('[data-clause]')).toHaveLength(1))
    const [front, back] = [...preview.querySelectorAll<HTMLElement>('section[data-face]')]
    expect(front?.querySelector('[data-clause]')).toBeNull()
    expect(back?.querySelector('[data-clause]')).toHaveTextContent("Damage can't be prevented this turn.")
    expect(within(preview).getByRole('listitem')).toHaveTextContent(
      'Back face: the engine does not run damage prevention yet.',
    )
  })

  it('says a card with nothing left to the table is fully supported, and underlines nothing', async () => {
    const { user } = await renderBuilder({ cards: { details: [boltDetail()] } })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    expect(await within(preview).findByText(/Fully supported/)).toBeInTheDocument()
    expect(preview.querySelector('[data-clause]')).toBeNull()
    expect(within(preview).queryByRole('list')).toBeNull()
  })

  it('does not claim a flagged card is fully supported when the check names no clause', async () => {
    const { user } = await renderBuilder({
      cards: { details: [boltDetail({ card: { ...BOLT, support: 'stale' } })] },
    })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    expect(await within(preview).findByText(/does not say which words/)).toBeInTheDocument()
    expect(within(preview).queryByText(/Fully supported/)).toBeNull()
  })
})

describe('closing the preview', () => {
  it('closes on Escape and gives focus back to the name that opened it', async () => {
    const { user } = await renderBuilder({ cards: { details: [boltDetail()] } })
    const opener = nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt')
    await user.click(opener)
    await dialog('Lightning Bolt')
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    await waitFor(() => expect(opener).toHaveFocus())
  })

  it('closes on the Close button and returns focus to the name of a search result', async () => {
    const { user } = await renderBuilder({ cards: { details: [boltDetail()] } })
    await user.type(searchBox(), 'bolt')
    const opener = await within(searchPane()).findByRole('button', { name: 'Lightning Bolt' })
    await user.click(opener)
    const preview = await dialog('Lightning Bolt')
    await user.click(within(preview).getByRole('button', { name: 'Close' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    await waitFor(() => expect(opener).toHaveFocus())
  })

  it('opens another card afresh after one closed', async () => {
    const { user } = await renderBuilder({
      cards: {
        details: [
          boltDetail(),
          cardDetail('Duress', { card: { ...cardHit('Duress'), oracle_id: 'oracle-duress' } }),
        ],
      },
    })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    await dialog('Lightning Bolt')
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    await user.click(nameButton(deckLine('Duress'), 'Duress'))
    const second = await dialog('Duress')
    expect(second).not.toHaveTextContent(BOLT_TEXT)
  })

  it('is a sheet from the bottom under 640 px and a dialog in the middle from there up', async () => {
    const { user } = await renderBuilder({ cards: { details: [boltDetail()] } })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    expect(preview).toHaveClass('max-sm:bottom-0', 'max-sm:w-full', 'max-sm:rounded-b-none', 'sm:max-w-xl')
  })
})

describe('the states of the preview', () => {
  it('shows the card it already knows while the rest loads, and cancels the request when closed', async () => {
    const { user, cards } = await renderBuilder({
      cards: { override: { get: () => new Promise<Response>(() => undefined) } },
    })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    expect(within(preview).getByText('Loading the card…')).toBeInTheDocument()
    expect(within(preview).getByText('Instant')).toBeInTheDocument()
    expect(preview.querySelector('[data-support="verified"]')).not.toBeNull()
    const signal = cards.calls.at(-1)?.init?.signal
    expect(signal?.aborted).toBe(false)
    await user.keyboard('{Escape}')
    await waitFor(() => expect(signal?.aborted).toBe(true))
  })

  it('says why it failed and tries again on request', async () => {
    let asked = 0
    const { user } = await renderBuilder({
      cards: {
        override: {
          get: () => {
            asked += 1
            return asked === 1 ? deckError(500, 'internal', 'boom') : jsonResponse(200, boltDetail())
          },
        },
      },
    })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    expect(await within(preview).findByRole('alert')).toHaveTextContent('The server ran into a problem.')
    await user.click(within(preview).getByRole('button', { name: 'Try again' }))
    await waitFor(() => expect(oracle(preview)).toHaveTextContent(BOLT_TEXT))
    expect(within(preview).queryByRole('alert')).toBeNull()
    expect(asked).toBe(2)
  })

  it('says a server without a card database cannot give details, with no retry', async () => {
    const { user } = await renderBuilder({ cards: { unavailable: true } })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    expect(await within(preview).findByText(/card details are not available/)).toBeInTheDocument()
    expect(within(preview).queryByRole('button', { name: 'Try again' })).toBeNull()
  })

  it('says a card the database does not hold is not there', async () => {
    const { user } = await renderBuilder({ cards: { details: [] } })
    await user.click(nameButton(deckLine('Lightning Bolt'), 'Lightning Bolt'))
    const preview = await dialog('Lightning Bolt')
    expect(await within(preview).findByRole('alert')).toHaveTextContent('That card is not in the database.')
  })
})
