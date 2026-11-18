// Goal: a relic shows its rarity (the word in its tier's colour) wherever it appears (relic pick, shop, Surrender, the run header), a bargain's price is paid in its own scene (Discard: choose a card, then confirm; Surrender: give up one relic; neither can be skipped), banes read as banes and a moved life cap shows beside the life.
import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TooltipProvider } from '@/components/ui/tooltip'
import type { CustomCardView, Offer, Pick, Rarity } from '@/protocol/generated'
import { renderInRouter } from '@/test-utils/router'
import { CustomCardFrame } from './CustomCardFrame'
import { DelveOffer } from './DelveOffer'
import { CONTENT, delveView, hero } from './fakeDelves'
import { RARITY_TONE } from './rarity'
import { RunHeader } from './RunHeader'
import { isBane, upgradeWords } from './upgrades'

function offer(shown: Offer, mirror: readonly string[] = [], gold = 100) {
  const onPick = vi.fn<(pick: Pick) => void>()
  render(
    <DelveOffer
      offer={shown}
      gold={gold}
      life={20}
      content={CONTENT}
      busy={false}
      find={async () => null}
      mirror={mirror}
      onPick={onPick}
    />,
  )
  return onPick
}

const relicOf = (rarity: Rarity, role = ''): CustomCardView => ({
  id: `r-${rarity}`,
  name: `${rarity} Charm`,
  kind: 'Relic',
  type_line: 'Relic',
  text: 'Something.',
  art: 'charm',
  role,
  price: 50,
  rarity,
})

describe('rarity', () => {
  it.each<Rarity>(['Common', 'Rare', 'Mythic'])('frames a %s relic with its word and colour', (rarity) => {
    render(<CustomCardFrame card={relicOf(rarity)} />)
    const figure = screen.getByRole('figure', { name: `${rarity} Charm` })
    expect(figure).toHaveAttribute('data-rarity', rarity)
    expect(figure).toHaveClass(RARITY_TONE[rarity].border)
    const word = within(figure).getByText(rarity)
    expect(word).toHaveClass(RARITY_TONE[rarity].text)
  })

  it('keeps the Dangerous mark on a risky relic beside its rarity', () => {
    render(<CustomCardFrame card={relicOf('Mythic', 'Risky')} />)
    const figure = screen.getByRole('figure', { name: 'Mythic Charm' })
    expect(within(figure).getByText('Dangerous')).toBeInTheDocument()
    expect(within(figure).getByText('Mythic')).toBeInTheDocument()
    expect(figure).toHaveClass('border-destructive')
  })

  it('shows no rarity on a boss innate', () => {
    render(<CustomCardFrame card={{ ...relicOf('Rare'), kind: 'Innate', name: 'Skrix' }} />)
    expect(screen.getByRole('figure', { name: 'Skrix' })).not.toHaveAttribute('data-rarity')
    expect(screen.queryByText('Rare')).toBeNull()
  })

  it('names the rarity of each relic picked and sold', () => {
    offer({ Relics: { relics: ['idol', 'pact'] } })
    expect(within(screen.getByRole('figure', { name: 'Ember Idol' })).getByText('Rare')).toBeInTheDocument()
    expect(
      within(screen.getByRole('figure', { name: 'Wildfire Pact' })).getByText('Mythic'),
    ).toBeInTheDocument()
  })

  it('names the rarity of a relic at the shop', () => {
    offer({ Shop: { items: [{ Relic: { id: 'idol', price: 70, sold: false } }] } })
    expect(within(screen.getByRole('figure', { name: 'Ember Idol' })).getByText('Rare')).toBeInTheDocument()
  })

  it('edges each relic of the run header in its rarity and names it', async () => {
    const view = delveView({}, { status: 'Map', heroes: [hero({ relics: ['idol', 'pact'] })] })
    await renderInRouter(
      <TooltipProvider>
        <RunHeader view={view} content={CONTENT} />
      </TooltipProvider>,
    )
    const [idol, pact] = within(screen.getByRole('group', { name: 'Relics of Ana' })).getAllByRole('button')
    expect(idol).toHaveAttribute('data-rarity', 'Rare')
    expect(idol).toHaveClass(RARITY_TONE.Rare.border)
    expect(idol).toHaveAccessibleName(/^Ember Idol, Rare,/)
    expect(pact).toHaveAttribute('data-rarity', 'Mythic')
    expect(pact).toHaveAccessibleName(/^Wildfire Pact, Mythic, risky,/)
  })
})

describe('a bargain’s price', () => {
  it('gives up a card chosen from the deck and Satchel only once confirmed, with no way to skip', () => {
    const onPick = offer('Discard', ['Cultivate', 'Opt', 'Sol Ring'])
    const confirm = screen.getByRole('button', { name: 'Choose a card to give up' })
    expect(confirm).toBeDisabled()
    const list = screen.getByRole('list', { name: 'Cards to give up' })
    fireEvent.change(screen.getByRole('searchbox', { name: 'Find a card' }), { target: { value: 'op' } })
    expect(within(list).getAllByRole('button')).toHaveLength(1)
    fireEvent.click(screen.getByRole('button', { name: 'Choose Opt' }))
    expect(onPick).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'Choose Opt' })).toHaveAttribute('aria-pressed', 'true')
    fireEvent.click(screen.getByRole('button', { name: 'Give up Opt' }))
    expect(onPick).toHaveBeenCalledExactlyOnceWith({ Discard: 'Opt' })
    expect(screen.queryByRole('button', { name: /skip|leave|none/i })).toBeNull()
  })

  it('gives up one relic, shown with its rarity, with no way to skip', () => {
    const onPick = offer({ Surrender: { relics: ['idol', 'pact'] } })
    const scene = screen.getByRole('region', { name: 'Give up a relic' })
    expect(within(scene).getByText('Mythic')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Give up Wildfire Pact' }))
    expect(onPick).toHaveBeenCalledExactlyOnceWith({ Take: 1 })
    expect(within(scene).getAllByRole('button')).toHaveLength(2)
  })
})

describe('banes and the life cap', () => {
  it.each([
    [{ Tax: 1 }, 'Ruric costs {1} more to cast'],
    [{ Shrink: 2 }, 'Ruric gets −2/−2'],
  ] as const)('reads %j as a bane: %s', (boon, words) => {
    const upgrade = { commander: 'Ruric', boon }
    expect(upgradeWords(upgrade)).toBe(words)
    expect(isBane(upgrade)).toBe(true)
  })

  it('tells a boon from a bane', () => {
    expect(isBane({ commander: 'Ruric', boon: { Pump: 1 } })).toBe(false)
    expect(isBane({ commander: 'Ruric', boon: 'Counter' })).toBe(false)
  })

  it('lists banes in the header apart from upgrades, with the cut life cap and the cards given up', async () => {
    const ana = hero({
      life: 30,
      life_cap: -5,
      lost: ['Opt'],
      upgrades: [
        { commander: 'Ruric', boon: { Pump: 1 } },
        { commander: 'Ruric', boon: { Tax: 1 } },
      ],
    })
    const view = delveView({}, { status: 'Map', heroes: [ana, hero({ name: 'Bo' })] })
    await renderInRouter(
      <TooltipProvider>
        <RunHeader view={view} content={CONTENT} />
      </TooltipProvider>,
    )
    const rows = [...document.querySelectorAll('[data-slot="hero-upgrade"]')]
    expect(rows.map((r) => r.getAttribute('data-bane'))).toEqual([null, 'true'])
    expect(rows[1]).toHaveClass('text-destructive')
    expect(rows[1]).toHaveTextContent('Bane: Ruric costs {1} more to cast')
    const life = screen.getByRole('img', { name: 'Ana: 30 life of a 35 cap' })
    expect(within(life).getByText('/35')).toHaveClass('text-destructive')
    expect(screen.getByRole('img', { name: 'Bo: 40 life' })).not.toHaveTextContent('/')
    expect(screen.getByText('Given up: Opt')).toBeInTheDocument()
  })
})
