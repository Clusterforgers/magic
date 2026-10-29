// Goal: the search pane says how to start until something is typed or chosen, sends one request when typing pauses and lists each hit with name, cost and type, adds a copy to the main deck by click or Enter and to the other zones from a menu, sends each filter the way the server reads it, pages 40 at a time and starts over when the search changes, gives every control a 44 px target under a coarse pointer, and designs its loading, empty, failed and unavailable states.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { deckError } from '@/features/decks/fakeDecks'
import { flushFakeClock, installFakeClock } from '@/test-utils/fakeClock'
import { pool } from './fakeCards'
import {
  asked,
  BOLT,
  CHAIN,
  copies,
  deckPane,
  GROWTH,
  renderBuilder,
  searchBox,
  searchPane,
} from './renderBuilder'

// A fake clock: the search waits out a pause in typing only when a test waits, never because the machine is slow.
beforeEach(installFakeClock)
afterEach(flushFakeClock)

const results = () => screen.findByRole('list', { name: 'Search results' })

describe('before anything is typed', () => {
  it('says how to start and asks the server nothing', async () => {
    const { cards } = await renderBuilder()
    expect(within(searchPane()).getByRole('heading', { name: 'Find cards to add' })).toBeInTheDocument()
    expect(searchBox()).toHaveValue('')
    expect(cards.calls).toEqual([])
  })
})

describe('typing', () => {
  it('sends one request for the whole word once typing pauses', async () => {
    const { user, cards } = await renderBuilder()
    await user.type(searchBox(), 'lightning')
    await results()
    expect(asked(cards.searches)).toEqual([{ q: 'lightning', page: '1' }])
  })

  it('lists each hit with its name, its cost as symbols and its type', async () => {
    const { user } = await renderBuilder()
    await user.type(searchBox(), 'lightning')
    const rows = within(await results()).getAllByRole('listitem')
    expect(rows.map((row) => row.getAttribute('data-card'))).toEqual([BOLT.oracle_id, CHAIN.oracle_id])
    expect(rows[0]).toHaveTextContent('Lightning Bolt')
    expect(rows[0]).toHaveTextContent('Instant')
    expect(within(rows[0] as HTMLElement).getByRole('img', { name: 'Mana cost: red' })).toBeInTheDocument()
    expect(rows[1]).toHaveTextContent('Sorcery')
  })

  it('says how many copies of a hit the deck already holds', async () => {
    const { user } = await renderBuilder()
    await user.type(searchBox(), 'bolt')
    const row = within(await results()).getAllByRole('listitem')[0] as HTMLElement
    expect(row).toHaveTextContent('4 in deck')
    await user.click(within(row).getByRole('button', { name: 'Add Lightning Bolt to main deck' }))
    expect(row).toHaveTextContent('5 in deck')
  })
})

describe('adding', () => {
  it('puts a copy in the main deck for each click on a row', async () => {
    const { user } = await renderBuilder()
    await user.type(searchBox(), 'chain')
    const add = await screen.findByRole('button', { name: 'Add Chain Lightning to main deck' })
    await user.click(add)
    expect(copies('Chain Lightning')).toBe(1)
    await user.click(add)
    expect(copies('Chain Lightning')).toBe(2)
  })

  it('searches at once on Enter while typing is still being waited out, and adds the first hit on the next Enter', async () => {
    const { user, cards } = await renderBuilder()
    await user.type(searchBox(), 'bolt{Enter}')
    await results()
    expect(cards.searches).toHaveLength(1)
    expect(copies('Lightning Bolt')).toBe(4)
    await user.keyboard('{Enter}')
    expect(copies('Lightning Bolt')).toBe(5)
    await user.keyboard('{Enter}')
    expect(copies('Lightning Bolt')).toBe(6)
    expect(searchBox()).toHaveFocus()
  })

  it('adds to the sideboard or the companion slot from the menu of a row', async () => {
    const { user } = await renderBuilder()
    await user.type(searchBox(), 'growth')
    await user.click(
      await screen.findByRole('button', { name: 'Add Giant Growth to another part of the deck' }),
    )
    expect((await screen.findAllByRole('menuitem')).map((item) => item.textContent)).toEqual([
      'Sideboard',
      'Companion',
    ])
    await user.click(screen.getByRole('menuitem', { name: 'Sideboard' }))
    expect(copies('Giant Growth', 'sideboard')).toBe(1)
    expect(within(deckPane()).getByRole('heading', { name: /^Sideboard/ })).toHaveTextContent('3')
  })

  it('offers the command zone in Commander and nowhere else but Freeform', async () => {
    const { user } = await renderBuilder({ view: { format: 'commander' } })
    await user.type(searchBox(), 'growth')
    await user.click(
      await screen.findByRole('button', { name: 'Add Giant Growth to another part of the deck' }),
    )
    expect((await screen.findAllByRole('menuitem')).map((item) => item.textContent)).toEqual([
      'Commander',
      'Sideboard',
      'Companion',
    ])
    await user.click(screen.getByRole('menuitem', { name: 'Commander' }))
    await waitFor(() => expect(deckPane().querySelector('[data-commander="oracle-growth"]')).not.toBeNull())
  })

  it('says aloud what was added', async () => {
    const { user } = await renderBuilder()
    await user.type(searchBox(), 'growth')
    await user.click(await screen.findByRole('button', { name: 'Add Giant Growth to main deck' }))
    expect(screen.getByText('Added Giant Growth to the main deck: 1 copy.')).toBeInTheDocument()
  })
})

describe('the filters', () => {
  it('send colour identity as letters in WUBRG order, and a toggle that is on says so', async () => {
    const { user, cards } = await renderBuilder()
    const pane = within(searchPane())
    await user.click(pane.getByRole('button', { name: 'Green' }))
    await user.click(pane.getByRole('button', { name: 'White' }))
    expect(pane.getByRole('button', { name: 'Green' })).toHaveAttribute('aria-pressed', 'true')
    expect(pane.getByRole('button', { name: 'Blue' })).toHaveAttribute('aria-pressed', 'false')
    await results()
    expect(asked(cards.searches).at(-1)).toEqual({ identity: 'WG', page: '1' })
    await user.click(pane.getByRole('button', { name: 'Green' }))
    await user.click(pane.getByRole('button', { name: 'White' }))
    await screen.findByRole('heading', { name: 'Find cards to add' })
  })

  it('send the type words, the mana value and the format with the text, trimmed', async () => {
    const { user, cards } = await renderBuilder()
    const pane = within(searchPane())
    await user.type(searchBox(), ' elves ')
    await user.type(pane.getByLabelText('Type'), 'creature elf')
    await user.type(pane.getByLabelText('Mana value'), '1')
    await user.selectOptions(pane.getByLabelText('Format'), 'modern')
    await results()
    expect(asked(cards.searches).at(-1)).toEqual({
      q: 'elves',
      type: 'creature elf',
      cmc: '1',
      format: 'modern',
      page: '1',
    })
  })

  it('search on a filter alone and give only the hits it matches', async () => {
    const { user } = await renderBuilder()
    await user.click(within(searchPane()).getByRole('button', { name: 'Green' }))
    const rows = within(await results()).getAllByRole('listitem')
    expect(rows.map((row) => row.getAttribute('data-card'))).toContain(GROWTH.oracle_id)
    expect(rows.map((row) => row.getAttribute('data-card'))).not.toContain(BOLT.oracle_id)
  })

  it('say in words what is wrong with a mana value and leave it out of the request', async () => {
    const { user, cards } = await renderBuilder()
    const pane = within(searchPane())
    await user.type(searchBox(), 'bolt')
    await user.type(pane.getByLabelText('Mana value'), '2-')
    expect(pane.getByLabelText('Mana value')).toHaveAttribute('aria-invalid', 'true')
    expect(pane.getByText(/Use a number, a range such as 2-4/)).toBeInTheDocument()
    await results()
    expect(asked(cards.searches).at(-1)).toEqual({ q: 'bolt', page: '1' })
    await user.type(pane.getByLabelText('Mana value'), '4')
    expect(pane.queryByText(/Use a number/)).toBeNull()
    await waitFor(() => expect(asked(cards.searches).at(-1)).toEqual({ q: 'bolt', cmc: '2-4', page: '1' }))
  })

  it('offer every format but Freeform, which has no legality to filter by', async () => {
    await renderBuilder()
    const options = within(within(searchPane()).getByLabelText('Format')).getAllByRole('option')
    expect(options[0]).toHaveTextContent('Any format')
    expect(options.map((option) => option.textContent)).toContain('Commander')
    expect(options.map((option) => option.textContent)).not.toContain('Freeform')
  })
})

describe('paging', () => {
  it('shows 40 hits to a page and moves with Previous and Next, which stop at the ends', async () => {
    const { user, cards } = await renderBuilder({ cards: { cards: pool(45) } })
    await user.type(searchBox(), 'card')
    await screen.findByText('1–40 of 45')
    const next = () => within(searchPane()).getByRole('button', { name: /Next/ })
    const previous = () => within(searchPane()).getByRole('button', { name: /Previous/ })
    expect(previous()).toHaveAttribute('aria-disabled', 'true')
    expect(within(await results()).getAllByRole('listitem')).toHaveLength(40)
    await user.click(next())
    await screen.findByText('41–45 of 45')
    expect(asked(cards.searches).at(-1)).toMatchObject({ page: '2' })
    expect(within(await results()).getAllByRole('listitem')).toHaveLength(5)
    expect(next()).toHaveAttribute('aria-disabled', 'true')
    await user.click(next())
    expect(asked(cards.searches)).toHaveLength(2)
    await user.click(previous())
    await screen.findByText('1–40 of 45')
  })

  it('starts over at page 1 when the text changes', async () => {
    const { user, cards } = await renderBuilder({ cards: { cards: pool(45) } })
    await user.type(searchBox(), 'card')
    await screen.findByText('1–40 of 45')
    await user.click(within(searchPane()).getByRole('button', { name: /Next/ }))
    await screen.findByText('41–45 of 45')
    await user.type(searchBox(), ' 0')
    await screen.findByText('1–40 of 45')
    expect(asked(cards.searches).at(-1)).toMatchObject({ q: 'card 0', page: '1' })
  })
})

describe('the states', () => {
  it('says when no card matches and clears the search from there', async () => {
    const { user } = await renderBuilder()
    await user.type(searchBox(), 'zzz')
    expect(await screen.findByRole('heading', { name: 'No cards match' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Clear search' }))
    expect(await screen.findByRole('heading', { name: 'Find cards to add' })).toBeInTheDocument()
    expect(searchBox()).toHaveValue('')
  })

  it('says it is searching while an answer is on its way', async () => {
    const { user } = await renderBuilder({
      cards: { override: { search: () => new Promise<Response>(() => undefined) } },
    })
    await user.type(searchBox(), 'bolt')
    expect((await screen.findAllByText('Searching cards…')).length).toBeGreaterThan(0)
    expect(screen.queryByRole('list', { name: 'Search results' })).toBeNull()
  })

  it('keeps the hits it has, dimmed, while the next page loads', async () => {
    let held = false
    const { user } = await renderBuilder({
      cards: {
        cards: pool(45),
        override: {
          search: ({ url }) =>
            held && url.includes('page=2') ? new Promise<Response>(() => undefined) : undefined,
        },
      },
    })
    await user.type(searchBox(), 'card')
    await screen.findByText('1–40 of 45')
    held = true
    await user.click(within(searchPane()).getByRole('button', { name: /Next/ }))
    const list = await results()
    expect(list).toHaveAttribute('aria-busy', 'true')
    expect(within(list).getAllByRole('listitem')).toHaveLength(40)
  })

  it('words a failed search, keeps the deck, and tries again on Retry', async () => {
    let attempts = 0
    const { user } = await renderBuilder({
      cards: {
        override: { search: () => (++attempts === 1 ? deckError(500, 'internal', 'boom') : undefined) },
      },
    })
    await user.type(searchBox(), 'bolt')
    const alert = await within(searchPane()).findByRole('alert')
    expect(alert).toHaveTextContent('Could not search the cards')
    expect(alert).toHaveTextContent('The server ran into a problem')
    await user.click(within(alert).getByRole('button', { name: 'Retry' }))
    expect(await results()).toBeInTheDocument()
    expect(attempts).toBe(2)
  })

  it('tells a server without a card database from a failure, and the deck can still be changed', async () => {
    const { user } = await renderBuilder({ cards: { unavailable: true } })
    await user.type(searchBox(), 'bolt')
    expect(await screen.findByText('Card search is not available')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Remove one Lightning Bolt' }))
    expect(copies('Lightning Bolt')).toBe(3)
  })
})

describe('targets', () => {
  it('are 44 px under a coarse pointer for the box, the filters, the rows and the pager', async () => {
    const { user } = await renderBuilder({ cards: { cards: pool(45) } })
    const pane = within(searchPane())
    expect(searchBox()).toHaveClass('pointer-coarse:h-11')
    expect(pane.getByLabelText('Type')).toHaveClass('pointer-coarse:h-11')
    expect(pane.getByLabelText('Mana value')).toHaveClass('pointer-coarse:h-11')
    expect(pane.getByLabelText('Format')).toHaveClass('pointer-coarse:h-11')
    expect(pane.getByRole('button', { name: 'Red' })).toHaveClass('pointer-coarse:size-11')
    await user.type(searchBox(), 'card')
    await screen.findByText('1–40 of 45')
    expect(pane.getAllByRole('button', { name: /^Add Card 001 to main deck/ })[0]?.parentElement).toHaveClass(
      'min-h-11',
    )
    expect(pane.getByRole('button', { name: 'Add Card 001 to another part of the deck' })).toHaveClass(
      'pointer-coarse:size-11',
    )
    expect(pane.getByRole('button', { name: /Next/ })).toHaveClass('pointer-coarse:h-11')
  })
})
