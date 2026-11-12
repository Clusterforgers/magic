// Goal: in the builder a card's picture (a tile, a commander) and its preview open the art picker; a choice for a line the server holds is stored at once and stays saved, one for a line not saved yet stays in the draft, Default clears it, "all my decks" stores the preferred art, and lines without art of their own draw the preferred one.
import { screen, waitFor, within } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { lineOf } from '@/features/decks/fakeDecks'
import { printingView, startPrintings } from '@/features/cards/fakePrintings'
import { COMMANDER_CHECK, COMMANDER_POOL, KRENKO } from './commanderFixture'
import { BOLT, CHAIN, deckLine, renderBuilder } from './renderBuilder'

const BOLT_ART = {
  [BOLT.oracle_id]: [
    printingView('p-new', 'sld', { set_name: 'Secret Lair Drop', released_at: '2024-02-01', full_art: true }),
    printingView('p-old', 'lea', {
      set_name: 'Limited Edition Alpha',
      released_at: '1993-08-05',
      frame: '1993',
    }),
  ],
}

const pictureOf = (row: HTMLElement): string | null =>
  row.querySelector('[data-slot="card-art"] img')?.getAttribute('src') ?? null

beforeEach(() => window.localStorage.clear())

describe('choosing the art of a deck line', () => {
  it('stores the choice at once, draws the line in it and leaves nothing unsaved', async () => {
    const art = startPrintings(BOLT_ART)
    const { user } = await renderBuilder({ look: 'pictures', printings: art.api })
    await user.click(
      within(deckLine('Lightning Bolt', 'main')).getByRole('button', { name: 'Art of Lightning Bolt' }),
    )
    const picker = await screen.findByRole('dialog', { name: 'Art for Lightning Bolt' })
    await user.click(await within(picker).findByRole('button', { name: /^Secret Lair Drop #1/ }))

    expect(art.lines).toEqual([
      { deckId: 'deck-1', zone: 'main', oracleId: BOLT.oracle_id, printingId: 'p-new' },
    ])
    expect(screen.queryByRole('dialog', { name: 'Art for Lightning Bolt' })).toBeNull()
    expect(pictureOf(deckLine('Lightning Bolt', 'main'))).toContain('/api/images/printing/p-new/')
    expect(screen.getByText('All changes saved.')).toBeInTheDocument()
  })

  it('marks the current art and clears it with Default', async () => {
    const art = startPrintings(BOLT_ART)
    const { user } = await renderBuilder({
      look: 'pictures',
      printings: art.api,
      view: { cards: [lineOf('Lightning Bolt', 4, { card: BOLT, printing_id: 'p-old' })] },
    })
    expect(pictureOf(deckLine('Lightning Bolt', 'main'))).toContain('/printing/p-old/')
    await user.click(screen.getByRole('button', { name: 'Art of Lightning Bolt' }))
    const picker = await screen.findByRole('dialog', { name: 'Art for Lightning Bolt' })
    expect(await within(picker).findByRole('button', { name: /^Limited Edition Alpha/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    await user.click(within(picker).getByRole('button', { name: 'Default' }))
    expect(art.lines.at(-1)?.printingId).toBeNull()
    expect(pictureOf(deckLine('Lightning Bolt', 'main'))).toContain('/api/images/oracle/')
  })

  it('keeps the art of a line not saved yet in the draft, for the next save', async () => {
    const art = startPrintings({ [CHAIN.oracle_id]: [printingView('p-chain', 'sld')] })
    const { user } = await renderBuilder({ look: 'pictures', printings: art.api })
    await user.type(screen.getByRole('searchbox', { name: 'Search cards' }), 'Chain')
    await user.click(await screen.findByRole('button', { name: 'Add Chain Lightning to main deck' }))
    await user.click(screen.getByRole('button', { name: 'Art of Chain Lightning' }))
    await user.click(await screen.findByRole('button', { name: /^SLD #1/ }))
    expect(art.lines).toEqual([])
    expect(pictureOf(deckLine('Chain Lightning', 'main'))).toContain('/printing/p-chain/')
  })

  it('stores the preferred art when asked, and draws it for lines without their own', async () => {
    const art = startPrintings(BOLT_ART)
    const { user, unmount } = await renderBuilder({ look: 'pictures', printings: art.api })
    await user.click(screen.getByRole('button', { name: 'Art of Lightning Bolt' }))
    const picker = await screen.findByRole('dialog', { name: 'Art for Lightning Bolt' })
    await user.click(within(picker).getByRole('checkbox', { name: 'Use this art in all my decks' }))
    await user.click(await within(picker).findByRole('button', { name: /^Limited Edition Alpha/ }))
    expect(art.preferred).toEqual({ [BOLT.oracle_id]: 'p-old' })

    const again = startPrintings(BOLT_ART, { preferred: { [BOLT.oracle_id]: 'p-new' } })
    unmount()
    await renderBuilder({ look: 'pictures', printings: again.api })
    await screen.findByRole('button', { name: 'Art of Lightning Bolt' })
    await expect.poll(() => pictureOf(deckLine('Lightning Bolt', 'main'))).toContain('/printing/p-new/')
  })

  it('shows a failed save in the picker and keeps the art as it was', async () => {
    const art = startPrintings(BOLT_ART, { failLine: 'The printing is not one of the card.' })
    const { user } = await renderBuilder({ look: 'pictures', printings: art.api })
    await user.click(screen.getByRole('button', { name: 'Art of Lightning Bolt' }))
    const picker = await screen.findByRole('dialog', { name: 'Art for Lightning Bolt' })
    await user.click(await within(picker).findByRole('button', { name: /^Secret Lair Drop/ }))
    expect(within(picker).getByRole('alert')).toHaveTextContent('The printing is not one of the card.')
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Art for Lightning Bolt' })).toBeNull())
    expect(pictureOf(deckLine('Lightning Bolt', 'main'))).toContain('/api/images/oracle/')
  })

  it('opens from the preview of a deck card', async () => {
    const art = startPrintings(BOLT_ART)
    const { user } = await renderBuilder({ look: 'pictures', printings: art.api })
    await user.click(
      within(deckLine('Lightning Bolt', 'main')).getByRole('button', { name: 'Lightning Bolt' }),
    )
    const preview = await screen.findByRole('dialog', { name: 'Lightning Bolt' })
    await user.click(within(preview).getByRole('button', { name: 'Choose art' }))
    expect(await screen.findByRole('dialog', { name: 'Art for Lightning Bolt' })).toBeInTheDocument()
  })

  it('chooses the art of a commander from its picture in the slot', async () => {
    const art = startPrintings({ [KRENKO.oracle_id]: [printingView('p-krenko', 'm13')] })
    const { user } = await renderBuilder({
      printings: art.api,
      view: { format: 'commander', cards: [lineOf(KRENKO.name, 1, { zone: 'commander', card: KRENKO })] },
      cards: { cards: COMMANDER_POOL },
      decks: { pool: COMMANDER_POOL },
      check: COMMANDER_CHECK,
    })
    const slot = document.querySelector<HTMLElement>('[data-slot="commander-slot"]')
    if (slot === null) throw new Error('no commander slot')
    await user.click(within(slot).getByRole('button', { name: `Art of ${KRENKO.name}` }))
    await user.click(await screen.findByRole('button', { name: /^M13 #1/ }))
    expect(art.lines).toEqual([
      { deckId: 'deck-1', zone: 'commander', oracleId: KRENKO.oracle_id, printingId: 'p-krenko' },
    ])
    expect(pictureOf(slot)).toContain('/printing/p-krenko/')
  })

  it('offers no art without the art endpoints', async () => {
    await renderBuilder({ look: 'pictures' })
    expect(screen.queryByRole('button', { name: 'Art of Lightning Bolt' })).toBeNull()
  })
})
