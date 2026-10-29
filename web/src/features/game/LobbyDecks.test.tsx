// Goal: every named seat of the lobby shows its deck to the whole table as the deck's name, format and a count of cards that need manual steps (one, many or none), a seat without a deck says "no deck yet", an open seat and a page that has only the lookup say nothing about decks, the summary waits for a seat without a deck to choose one, and no card name ever reaches the page, whatever the roster carries.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { SeatDeck } from '@/protocol/generated'
import { DECKS } from '@/test-utils/decks'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { renderGame } from '@/test-utils/gameHarness'
import { lobbyMessage, SEAT_DECK } from '@/test-utils/lobbyFrames'
import type { SeatFlags } from '@/test-utils/lobbyFrames'
import { deckFormat, deckWords, issueWords, manualWords, sameDeck, stillWords } from './deckWords'
import { lobbyRows, mayReady, summaryText } from './lobbyModel'
import type { LobbyRow } from './lobbyModel'

const BURN: SeatDeck = { name: 'Burn', format: 'modern', manual_cards: 3 }
const MILL: SeatDeck = { name: 'Pauper Mill', format: 'pauper', manual_cards: 1 }

const rows = (): HTMLElement[] => within(screen.getByRole('list', { name: 'Seats' })).getAllByRole('listitem')
const deckLine = (row: HTMLElement): string | null =>
  row.querySelector('[data-slot="seat-deck"]')?.textContent ?? null

async function lobby(table: SeatFlags[], extra?: (seat: Record<string, unknown>, index: number) => void) {
  const h = await renderGame({
    script: {
      game: () =>
        jsonResponse(200, {
          started: false,
          seats: table.map(([name], seat) => ({ seat, name })),
          connected: [0],
          ready: [],
          result: null,
          visibility: 'public',
        }),
    },
  })
  h.join(0, table.length)
  const message = lobbyMessage(table)
  if (extra !== undefined)
    message.seats.forEach((seat, index) => extra(seat as unknown as Record<string, unknown>, index))
  h.deliver(message)
  await h.settle()
  return h
}

describe('what each seat shows the table', () => {
  const TABLE: SeatFlags[] = [
    ['Ann', true, false, SEAT_DECK],
    ['Bea', true, false, BURN],
    ['Cy', true, true, MILL],
    ['Di', true, false, null],
    ['', false, false],
  ]

  it('writes name, format and the count of manual cards for a seat that has a deck', async () => {
    await lobby(TABLE)
    expect(deckLine(rows()[0] as HTMLElement)).toBe('Deck: Mono Green Stompers, Normal, no manual steps')
    expect(deckLine(rows()[1] as HTMLElement)).toBe('Deck: Burn, Modern, 3 cards need manual steps')
    expect(deckLine(rows()[2] as HTMLElement)).toBe('Deck: Pauper Mill, Pauper, 1 card needs manual steps')
  })

  it('says "no deck yet" for a joined seat without one, and nothing for an open seat', async () => {
    await lobby(TABLE)
    expect(deckLine(rows()[3] as HTMLElement)).toBe('Deck: no deck yet')
    expect(rows()[3]?.querySelector('[data-slot="seat-deck"]')).toHaveAttribute('data-deck', 'none')
    expect(deckLine(rows()[4] as HTMLElement)).toBeNull()
    expect(rows()[4]).toHaveTextContent('not joined yet')
  })

  it('keeps each deck line with its own seat, beside the name and apart from the state', async () => {
    await lobby(TABLE)
    expect(rows()[1]).toHaveTextContent('Seat 2BeaDeck: Burn')
    expect(within(rows()[1] as HTMLElement).getByText('Not ready yet')).toBeInTheDocument()
    expect(rows()[2]?.querySelector('[data-slot="seat-state"]')).toHaveTextContent('Ready')
  })

  it('says nothing about decks while the lookup is all there is, since only the roster carries them', async () => {
    const h = await renderGame({
      script: {
        game: () =>
          jsonResponse(200, {
            started: false,
            seats: [
              { seat: 0, name: 'Ann' },
              { seat: 1, name: 'Bea' },
            ],
            connected: [0],
            ready: [],
            result: null,
            visibility: 'public',
          }),
      },
    })
    h.join(0, 2)
    await h.settle()
    expect(rows().map(deckLine)).toEqual([null, null])
    h.deliver(
      lobbyMessage([
        ['Ann', true, false],
        ['Bea', true, false, null],
      ]),
    )
    expect(rows().map(deckLine)).toEqual([
      'Deck: Mono Green Stompers, Normal, no manual steps',
      'Deck: no deck yet',
    ])
    h.fail()
    expect(rows().map(deckLine)).toEqual([null, null])
  })

  it('waits for the seats without a deck to choose one before it waits for Ready', async () => {
    await lobby(TABLE)
    expect(document.querySelector('[data-slot="lobby-summary"]')).toHaveTextContent(
      '1 of 5 ready, waiting for Di to choose a deck and you and Bea to press Ready and Seat 5 to join',
    )
  })
})

describe('no card name reaches the lobby', () => {
  // Every card of every demo deck, and cards a roster might carry by mistake.
  const NAMES = [
    ...DECKS.flatMap((deck) => deck.cards.map((card) => card.name)),
    'Lightning Bolt',
    'Goblin Guide',
  ]
  const mentions = (text: string): string[] => NAMES.filter((name) => text.includes(name))

  it('can tell: the scan finds a card in text that has one', () => {
    expect(mentions('Burn, Modern, with Lightning Bolt')).toEqual(['Lightning Bolt'])
    expect(mentions('Burn, Modern, 3 cards need manual steps')).toEqual([])
  })

  it('shows only the three facts when a roster carries more about a deck', async () => {
    await lobby(
      [
        ['Ann', true, false, BURN],
        ['Bea', true, false, MILL],
      ],
      (seat) => {
        const deck = seat.deck as Record<string, unknown>
        deck.cards = ['Lightning Bolt', 'Goblin Guide']
        deck.main = [{ name: 'Lightning Bolt', count: 4 }]
        deck.sideboard = ['Goblin Guide']
      },
    )
    expect(rows().map(deckLine)).toEqual([
      'Deck: Burn, Modern, 3 cards need manual steps',
      'Deck: Pauper Mill, Pauper, 1 card needs manual steps',
    ])
    expect(mentions(document.body.textContent ?? '')).toEqual([])
    expect(document.body.innerHTML).not.toContain('Lightning Bolt')
  })

  it('shows no card of the built-in decks in the roster or anywhere else on the lobby', async () => {
    await lobby([
      ['Ann', true, false, SEAT_DECK],
      ['Bea', true, false, { name: 'Mono Red Giants', format: 'normal', manual_cards: 0 }],
    ])
    await screen.findByRole('list', { name: 'Built-in decks' })
    const lobbyText = document.body.textContent ?? ''
    expect(mentions(lobbyText)).toEqual([])
    expect(
      mentions(
        rows()
          .map((row) => row.textContent)
          .join(' '),
      ),
    ).toEqual([])
  })
})

describe('the words for a deck', () => {
  it.each<[number, string]>([
    [0, 'no manual steps'],
    [1, '1 card needs manual steps'],
    [2, '2 cards need manual steps'],
    [60, '60 cards need manual steps'],
  ])('counts %i manual cards as "%s"', (count, words) => {
    expect(manualWords(count)).toBe(words)
  })

  it.each<[string, string]>([
    ['normal', 'Normal'],
    ['modern', 'Modern'],
    ['commander', 'Commander'],
    ['penny', 'Penny Dreadful'],
    ['something-new', 'something-new'],
  ])('names the format %s as %s', (key, name) => {
    expect(deckFormat(key)).toBe(name)
  })

  it('spells name, format and count in one phrase, and tells decks apart by all three', () => {
    expect(deckWords(BURN)).toBe('Burn, Modern, 3 cards need manual steps')
    expect(sameDeck(BURN, { ...BURN })).toBe(true)
    expect(sameDeck(BURN, { ...BURN, manual_cards: 2 })).toBe(false)
    expect(sameDeck(BURN, { ...BURN, format: 'legacy' })).toBe(false)
    expect(sameDeck(BURN, MILL)).toBe(false)
  })

  it.each<[SeatDeck | null | undefined, string]>([
    [BURN, 'Your deck is still Burn, Modern, 3 cards need manual steps.'],
    [null, 'You still have no deck.'],
    [undefined, 'Your deck has not changed.'],
  ])('says what is still true of %j', (current, words) => {
    expect(stillWords(current)).toBe(words)
  })

  it.each<[string, string]>([
    ['the main deck has 58 cards', 'The main deck has 58 cards.'],
    ['  Lightning Bolt: five copies.  ', 'Lightning Bolt: five copies.'],
    ['Is it legal?', 'Is it legal?'],
    ['', 'The deck breaks a rule of the game.'],
  ])('turns the issue "%s" into the sentence "%s"', (message, words) => {
    expect(issueWords({ message })).toBe(words)
  })
})

describe('the rows of a lobby with decks', () => {
  const roster = lobbyMessage([
    ['Ann', true, false, BURN],
    ['Bea', true, false, null],
    ['', false, false],
  ])

  it('takes each seat’s deck from the roster, null for none and undefined without one', () => {
    const table = lobbyRows({
      roster: { seats: roster.seats, needed: roster.needed },
      info: null,
      open: true,
      seat: 0,
      seatCount: 3,
    })
    expect(table.map((row) => row.deck)).toEqual([BURN, null, null])
    const lookup = lobbyRows({ roster: null, info: null, open: true, seat: 0, seatCount: 3 })
    expect(lookup.map((row) => row.deck)).toEqual([undefined, undefined, undefined])
  })

  it.each<[SeatDeck | null | undefined, boolean]>([
    [BURN, true],
    [undefined, true],
    [null, false],
  ])('lets a seat with the deck %j press Ready: %s', (deck, may) => {
    expect(mayReady({ deck })).toBe(may)
  })

  const row = (seat: number, over: Partial<LobbyRow>): LobbyRow => ({
    seat,
    name: ['Ann', 'Bea', 'Cy'][seat] ?? `Seat ${seat + 1}`,
    named: true,
    you: false,
    connected: true,
    ready: false,
    deck: SEAT_DECK,
    ...over,
  })

  it.each<[string, LobbyRow[], string]>([
    ['everyone has a deck', [row(0, {}), row(1, {})], '0 of 2 ready, waiting for Ann and Bea to press Ready'],
    [
      'one is without',
      [row(0, {}), row(1, { deck: null })],
      '0 of 2 ready, waiting for Bea to choose a deck and Ann to press Ready',
    ],
    [
      'all are without',
      [row(0, { deck: null }), row(1, { deck: null }), row(2, { deck: null })],
      '0 of 3 ready, waiting for Ann, Bea and Cy to choose a deck',
    ],
    [
      'a seat without a deck is away',
      [row(0, {}), row(1, { deck: null, connected: false })],
      '0 of 2 ready, waiting for Ann to press Ready and Bea to join',
    ],
    [
      'the decks are not known',
      [row(0, { deck: undefined }), row(1, { deck: undefined })],
      '0 of 2 ready, waiting for Ann and Bea to press Ready',
    ],
  ])('sums up the table when %s', (_what, table, words) => {
    expect(summaryText(table)).toBe(words)
  })
})
