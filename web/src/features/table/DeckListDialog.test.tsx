// Goal: at every layout (desktop, short dock, phone on its side, phone upright) a seat's library chip opens the deck list the server told: a public list whole by group with its pictures and copies, a hidden deck's command zone alone with the commander's tokens, and a seat without one keeps a plain figure; the table menu opens the same lists, and the pure helpers group, count and word them.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ListCard, SeatList, TokenPicture } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { listSections, listWords, tokenLabel, tokenSource, tokenType } from './deckLists'

const line = (name: string, group: ListCard['group'], count = 1): ListCard => ({
  name,
  identity: { oracle_id: `o-${name}` },
  count,
  group,
})

const GOBLIN: TokenPicture = {
  name: 'Goblin',
  colors: ['Red'],
  types: ['Creature'],
  subtypes: ['Goblin'],
  power: 1,
  toughness: 1,
}
const TREASURE: TokenPicture = { name: 'Treasure', types: ['Artifact'], subtypes: ['Treasure'] }

const OWN: SeatList = { seat: 0, whole: true, cards: [line('Forest', 'Land', 40)], tokens: [] }
const PUBLIC: SeatList = {
  seat: 1,
  whole: true,
  cards: [
    line('Krenko, Mob Boss', 'Commander'),
    line('Goblin Instigator', 'Creature', 2),
    line('Smothering Tithe', 'Enchantment'),
    line('Mountain', 'Land', 30),
  ],
  tokens: [GOBLIN, TREASURE],
}
const HIDDEN: SeatList = {
  seat: 1,
  whole: false,
  cards: [line('Krenko, Mob Boss', 'Commander')],
  tokens: [GOBLIN],
}

const LAYOUTS = [
  ['desktop', { width: 1600, height: 1000 }],
  ['short dock', { width: 1280, height: 720 }],
  ['phone on its side', { width: 844, height: 390 }],
  ['phone', { width: 390, height: 844 }],
] as const

/** The library chips of seat 1: in its pod, or at the top of its board where a phone draws a tile instead. */
function libraryChips(container: HTMLElement): HTMLElement[] {
  return [
    ...container.querySelectorAll<HTMLElement>(
      '[data-seat="1"] [data-zone="library"], [data-slot="board-zones"] [data-zone="library"]',
    ),
  ]
}

function table(lists: SeatList[], viewport: { width: number; height: number }) {
  return renderTable({
    seats: 2,
    options: { viewport },
    state: (state) => ({ ...state, deckLists: lists }),
  })
}

describe('the deck list of a seat', () => {
  it.each(LAYOUTS)('opens a public list whole from the library chip (%s)', async (_, viewport) => {
    const { container, user } = table([OWN, PUBLIC], viewport)
    const chip = must(libraryChips(container).find((element) => element.tagName === 'BUTTON'))
    expect(chip).toHaveAttribute('data-list', 'whole')
    await user.click(chip)
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText(/’s deck$/)).toBeInTheDocument()
    expect(within(dialog).getByText('34 cards, a public list.')).toBeInTheDocument()
    const groups = within(dialog)
      .getAllByRole('region')
      .map((region) => region.getAttribute('aria-label'))
    expect(groups).toEqual([
      'Commander (1)',
      'Creatures (2)',
      'Enchantment (1)',
      'Lands (30)',
      expect.stringMatching(/^Tokens .* can make$/),
    ])
    expect(within(dialog).getByRole('listitem', { name: '2 Goblin Instigator' })).toBeInTheDocument()
    const tokens = dialog.querySelectorAll('[data-slot="deck-token-name"]')
    expect([...tokens].map((token) => token.textContent)).toEqual(['Goblin 1/1', 'Treasure'])
  })

  it.each(LAYOUTS)('shows a hidden deck’s command zone and its tokens alone (%s)', async (_, viewport) => {
    const { container, user } = table([OWN, HIDDEN], viewport)
    const chip = must(libraryChips(container).find((element) => element.tagName === 'BUTTON'))
    expect(chip).toHaveAttribute('data-list', 'public')
    expect(chip).toHaveAccessibleName(/open what is public of .*’s deck/)
    await user.click(chip)
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText(/list is hidden: only the command zone is public\.$/)).toBeInTheDocument()
    expect(dialog.querySelectorAll('[data-slot="deck-line"]')).toHaveLength(1)
    expect(dialog.querySelectorAll('[data-slot="deck-token"]')).toHaveLength(1)
  })

  it.each(LAYOUTS)(
    'keeps a plain library figure for a seat with nothing to look through (%s)',
    (_, viewport) => {
      const { container } = table([OWN], viewport)
      for (const chip of libraryChips(container)) expect(chip.tagName).not.toBe('BUTTON')
      // Only your own list opens; a phone's tile of the opponent keeps its count and draws no chip over the board.
      const buttons = [...container.querySelectorAll('button[data-zone="library"]')]
      expect(buttons.length).toBeGreaterThan(0)
      for (const button of buttons) expect(button.closest('[data-seat]')).toHaveAttribute('data-seat', '0')
    },
  )

  it('opens the lists from the table menu', async () => {
    const { user } = table([OWN, PUBLIC], { width: 1600, height: 1000 })
    await user.click(screen.getByRole('button', { name: 'Table menu' }))
    const decks = await screen.findByRole('group', { name: 'Deck lists' })
    const buttons = within(decks).getAllByRole('button')
    expect(buttons.map((button) => button.textContent)).toEqual([
      'Your deck',
      expect.stringMatching(/’s deck$/),
    ])
    await user.click(must(buttons[1]))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText('34 cards, a public list.')).toBeInTheDocument()
  })
})

describe('deckLists', () => {
  it('groups lines in table order, counting copies for the heading', () => {
    const sections = listSections([
      line('Mountain', 'Land', 2),
      line('Shock', 'Instant'),
      line('Bolt', 'Instant'),
    ])
    expect(sections.map((section) => [section.heading, section.copies])).toEqual([
      ['Instants', 2],
      ['Lands', 2],
    ])
  })

  it.each([
    [OWN, true, '40 cards, the list you play.'],
    [PUBLIC, false, '34 cards, a public list.'],
    [HIDDEN, false, 'Bo’s list is hidden: only the command zone is public.'],
  ])('words a list (%#)', (list, own, words) => {
    expect(listWords(list, 'Bo', own)).toBe(words)
  })

  it.each([
    [GOBLIN, 'Creature — Goblin', 'Goblin 1/1', ['R']],
    [TREASURE, 'Artifact — Treasure', 'Treasure', []],
    [
      { name: 'Jace', supertypes: ['Legendary'], types: ['Planeswalker'] } as TokenPicture,
      'Legendary Planeswalker',
      'Jace',
      [],
    ],
  ])('finds a token by the facts the table uses (%#)', (token, type, label, colors) => {
    expect(tokenType(token)).toBe(type)
    expect(tokenLabel(token)).toBe(label)
    expect(tokenSource(token)).toMatchObject({ kind: 'token', name: token.name, type, colors })
  })
})
