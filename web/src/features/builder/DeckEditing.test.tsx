// Goal: the deck pane lists the lines by zone with a total each, steps a line's copies up and down (the last copy removed takes the line out), removes a line outright, moves copies between zones onto the line already there, edits the name and the format, marks every change as unsaved and an undone change as saved again, stops at the card limit, and designs its empty states.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { lineOf } from '@/features/decks/fakeDecks'
import { absent, BOLT, copies, deckLine, deckPane, MOUNTAIN, renderBuilder, searchBox } from './renderBuilder'

const pane = () => within(deckPane())
const click = async (user: { click: (element: Element) => Promise<void> }, name: string | RegExp) =>
  user.click(screen.getByRole('button', { name }))

describe('the lines', () => {
  it('are listed by zone with the copies of each zone and of the deck', async () => {
    await renderBuilder()
    expect(pane().getByRole('heading', { name: 'Main deck 24' })).toBeInTheDocument()
    expect(pane().getByRole('heading', { name: 'Sideboard 2' })).toBeInTheDocument()
    expect(pane().queryByRole('heading', { name: /^Companion/ })).toBeNull()
    expect(
      within(pane().getByRole('list', { name: 'Main deck cards' })).getAllByRole('listitem'),
    ).toHaveLength(2)
    expect(
      within(pane().getByRole('list', { name: 'Sideboard cards' })).getAllByRole('listitem'),
    ).toHaveLength(1)
    expect(pane().getByText('26 cards')).toBeInTheDocument()
  })

  it('are in name order inside a zone, whatever order the deck was saved in', async () => {
    await renderBuilder()
    const names = within(pane().getByRole('list', { name: 'Main deck cards' }))
      .getAllByRole('listitem')
      .map((row) => row.getAttribute('data-line'))
    expect(names).toEqual([`main:${BOLT.oracle_id}`, `main:${MOUNTAIN.oracle_id}`])
  })

  it('show the cost as symbols and no cost for a land', async () => {
    await renderBuilder()
    expect(
      within(deckLine('Lightning Bolt')).getByRole('img', { name: 'Mana cost: red' }),
    ).toBeInTheDocument()
    expect(within(deckLine('Mountain')).queryByRole('img')).toBeNull()
  })
})

describe('plus and minus', () => {
  it('change the copies of a line by one, the total with them, and mark the deck unsaved', async () => {
    const { user } = await renderBuilder()
    expect(pane().queryByText('Unsaved changes')).toBeNull()
    await click(user, 'Add one Lightning Bolt')
    expect(copies('Lightning Bolt')).toBe(5)
    expect(pane().getByText('27 cards')).toBeInTheDocument()
    expect(pane().getByRole('heading', { name: 'Main deck 25' })).toBeInTheDocument()
    expect(pane().getByText('Unsaved changes')).toBeInTheDocument()
    await click(user, 'Remove one Lightning Bolt')
    await click(user, 'Remove one Lightning Bolt')
    expect(copies('Lightning Bolt')).toBe(3)
  })

  it('take the line out with its last copy, and the zone with its last line', async () => {
    const { user } = await renderBuilder()
    await click(user, 'Remove one Duress')
    expect(copies('Duress')).toBe(1)
    await click(user, 'Remove one Duress')
    expect(absent('Duress')).toBe(true)
    expect(pane().queryByRole('heading', { name: /^Sideboard/ })).toBeNull()
    expect(screen.getByText('Removed Duress from the sideboard.')).toBeInTheDocument()
  })

  it('say aloud nothing while a line keeps copies, and mark an undone change as saved again', async () => {
    const { user } = await renderBuilder()
    await click(user, 'Add one Lightning Bolt')
    expect(pane().getByText('Unsaved changes')).toBeInTheDocument()
    await click(user, 'Remove one Lightning Bolt')
    expect(pane().queryByText('Unsaved changes')).toBeNull()
    expect(pane().getByText('Manual play')).toBeInTheDocument()
  })
})

describe('remove', () => {
  it('takes a card out whatever its copies, and the deck pane says so aloud', async () => {
    const { user } = await renderBuilder()
    await click(user, 'Remove Lightning Bolt from the main deck')
    expect(absent('Lightning Bolt')).toBe(true)
    expect(pane().getByText('22 cards')).toBeInTheDocument()
    expect(screen.getByText('Removed Lightning Bolt from the main deck.')).toBeInTheDocument()
  })
})

describe('moving', () => {
  const menu = async (user: { click: (element: Element) => Promise<void> }, card: string) => {
    const trigger = within(deckLine(card, 'main')).getByRole('button', {
      name: `Move ${card} to another part of the deck`,
    })
    await user.click(trigger)
    return (await screen.findAllByRole('menuitem')).map((item) => item.textContent)
  }

  it('offers every other zone the format uses, all copies first and then one', async () => {
    const { user } = await renderBuilder()
    expect(await menu(user, 'Lightning Bolt')).toEqual([
      'All 4 to sideboard',
      'All 4 to companion',
      'One to sideboard',
      'One to companion',
    ])
  })

  it('offers the command zone in Commander, and only the move of the line when it has one copy', async () => {
    const { user } = await renderBuilder({
      view: { format: 'commander', cards: [lineOf('Mountain', 1, { card: MOUNTAIN })] },
    })
    expect(await menu(user, 'Mountain')).toEqual(['To commander', 'To sideboard', 'To companion'])
  })

  it('moves every copy to another zone and drops the empty line', async () => {
    const { user } = await renderBuilder()
    await menu(user, 'Lightning Bolt')
    await user.click(screen.getByRole('menuitem', { name: 'All 4 to sideboard' }))
    expect(copies('Lightning Bolt', 'sideboard')).toBe(4)
    expect(() => deckLine('Lightning Bolt', 'main')).toThrow()
    expect(pane().getByRole('heading', { name: 'Main deck 20' })).toBeInTheDocument()
    expect(pane().getByRole('heading', { name: 'Sideboard 6' })).toBeInTheDocument()
    expect(screen.getByText('Moved Lightning Bolt to sideboard.')).toBeInTheDocument()
  })

  it('moves one copy and leaves the rest', async () => {
    const { user } = await renderBuilder()
    await menu(user, 'Lightning Bolt')
    await user.click(screen.getByRole('menuitem', { name: 'One to sideboard' }))
    expect(copies('Lightning Bolt', 'main')).toBe(3)
    expect(copies('Lightning Bolt', 'sideboard')).toBe(1)
  })

  it('puts the copies onto the line the card already has in that zone', async () => {
    const { user } = await renderBuilder({
      view: {
        cards: [
          lineOf('Lightning Bolt', 3, { card: BOLT }),
          lineOf('Lightning Bolt', 1, { zone: 'sideboard', card: BOLT }),
        ],
      },
    })
    await menu(user, 'Lightning Bolt')
    await user.click(screen.getByRole('menuitem', { name: 'One to sideboard' }))
    expect(copies('Lightning Bolt', 'main')).toBe(2)
    expect(copies('Lightning Bolt', 'sideboard')).toBe(2)
    expect(
      within(pane().getByRole('list', { name: 'Sideboard cards' })).getAllByRole('listitem'),
    ).toHaveLength(1)
  })
})

describe('the name and the format', () => {
  it('can be changed, which marks the deck unsaved, and changed back, which does not', async () => {
    const { user } = await renderBuilder()
    const name = pane().getByRole('textbox', { name: 'Deck name' })
    const format = pane().getByRole('combobox', { name: 'Format' })
    expect(name).toHaveValue('Burn')
    expect(format).toHaveValue('modern')
    await user.type(name, ' v2')
    await user.selectOptions(format, 'legacy')
    expect(name).toHaveValue('Burn v2')
    expect(format).toHaveValue('legacy')
    expect(pane().getByText('Unsaved changes')).toBeInTheDocument()
    await user.clear(name)
    await user.type(name, 'Burn')
    await user.selectOptions(format, 'modern')
    expect(pane().queryByText('Unsaved changes')).toBeNull()
  })

  it('lists every deck format by name, and the format of the deck even when this page does not know it', async () => {
    await renderBuilder({ view: { format: 'brawl' } })
    const options = within(pane().getByRole('combobox', { name: 'Format' })).getAllByRole('option')
    expect(options[0]).toHaveValue('brawl')
    expect(options.map((option) => option.textContent)).toContain('Commander')
  })
})

describe('the card limit', () => {
  const full = { cards: [lineOf('Mountain', 250, { card: MOUNTAIN })] }

  it('disables plus on every line and says why', async () => {
    await renderBuilder({ view: full })
    expect(screen.getByRole('button', { name: 'Add one Mountain' })).toHaveAttribute('aria-disabled', 'true')
    expect(pane().getByText('The deck is at its limit of 250 cards.')).toBeInTheDocument()
  })

  it('refuses a card from the search and says so aloud', async () => {
    const { user } = await renderBuilder({ view: full })
    await user.type(searchBox(), 'bolt')
    const add = await screen.findByRole('button', { name: 'Add Lightning Bolt to main deck' })
    expect(add).toHaveAttribute('aria-disabled', 'true')
    await user.click(add)
    expect(absent('Lightning Bolt')).toBe(true)
    expect(copies('Mountain')).toBe(250)
  })

  it('does not hold a deck of 249 cards back from its 250th', async () => {
    const { user } = await renderBuilder({ view: { cards: [lineOf('Mountain', 249, { card: MOUNTAIN })] } })
    await click(user, 'Add one Mountain')
    expect(copies('Mountain')).toBe(250)
  })
})

describe('the empty states', () => {
  it('say what to do for a deck without cards', async () => {
    await renderBuilder({ view: { cards: [] } })
    expect(pane().getByRole('heading', { name: 'This deck has no cards' })).toBeInTheDocument()
    expect(pane().queryByRole('heading', { name: /^Main deck/ })).toBeNull()
    expect(pane().getByText('0 cards')).toBeInTheDocument()
  })

  it('ask for a commander in a Commander deck that has none', async () => {
    await renderBuilder({
      view: { format: 'commander', cards: [lineOf('Mountain', 20, { card: MOUNTAIN })] },
    })
    expect(pane().getByRole('heading', { name: 'Commander 0' })).toBeInTheDocument()
    expect(pane().getByText(/No commander yet/)).toBeInTheDocument()
  })
})

describe('targets and layout of the lines', () => {
  it('are 44 px under a coarse pointer for the name, the format, Save, the steppers and the menus', async () => {
    await renderBuilder()
    expect(pane().getByRole('textbox', { name: 'Deck name' })).toHaveClass('pointer-coarse:h-11')
    const format = pane().getByRole('combobox', { name: 'Format' })
    expect(format).toHaveClass('pointer-coarse:h-11')
    expect(format.getAttribute('data-size')).not.toBe('sm')
    expect(pane().getByRole('button', { name: 'Save' })).toHaveClass('pointer-coarse:h-11')
    for (const name of [
      'Add one Lightning Bolt',
      'Remove one Lightning Bolt',
      'Remove Lightning Bolt from the main deck',
    ]) {
      expect(pane().getByRole('button', { name })).toHaveClass('pointer-coarse:size-11')
    }
    expect(
      pane().getByRole('button', { name: 'Move Lightning Bolt to another part of the deck' }),
    ).toHaveClass('pointer-coarse:size-11')
  })

  it('wrap to two lines under 640 px and sit on one from there', async () => {
    await renderBuilder()
    const row = deckLine('Lightning Bolt')
    expect(row).toHaveClass('grid-cols-[minmax(0,1fr)_auto]', 'sm:grid-cols-[auto_minmax(0,1fr)_auto_auto]')
  })
})
