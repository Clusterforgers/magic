// Goal: the run page shows what the Satchel and commander upgrades do: a card pick and Reforge say the card joins the Satchel; the campfire offers Temper with each upgrade in words only when one fits; the shop sells an upgrade at its price; the Mirror Pool lists the cards it may copy, finds them by name and sends the copy or the skip; the Mirror Pool's event option names its effect; and the run header lists each hero's Satchel and upgrades.
import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TooltipProvider } from '@/components/ui/tooltip'
import type { Offer, Pick, Upgrade } from '@/protocol/generated'
import { renderInRouter } from '@/test-utils/router'
import { DelveOffer } from './DelveOffer'
import { chipOf } from './eventEffects'
import { CONTENT, delveView, hero } from './fakeDelves'
import { RunHeader } from './RunHeader'
import { satchelWords, UPGRADE_PRICE, upgradeWords } from './upgrades'

const noPictures = async () => null

function offer(shown: Offer, gold = 0, mirror: readonly string[] = []) {
  const onPick = vi.fn<(pick: Pick) => void>()
  render(
    <DelveOffer
      offer={shown}
      gold={gold}
      life={20}
      content={CONTENT}
      busy={false}
      find={noPictures}
      mirror={mirror}
      onPick={onPick}
    />,
  )
  return onPick
}

const UPGRADES: Upgrade[] = [
  { commander: 'Atraxa', boon: { Cheaper: 1 } },
  { commander: 'Atraxa', boon: { Keyword: 'FirstStrike' } },
  { commander: 'Atraxa', boon: { Pump: 2 } },
]

describe('upgrade words', () => {
  it.each<[Upgrade, string]>([
    [{ commander: 'A', boon: { Cheaper: 2 } }, 'A costs {2} less to cast'],
    [{ commander: 'A', boon: { Keyword: 'FirstStrike' } }, 'A has first strike'],
    [{ commander: 'A', boon: { Pump: 1 } }, 'A gets +1/+1'],
    [{ commander: 'A', boon: 'Counter' }, 'A enters with an additional +1/+1 counter'],
    [{ commander: 'A', boon: 'Loyalty' }, 'A enters with an additional loyalty counter'],
  ])('say %o as "%s"', (upgrade, words) => {
    expect(upgradeWords(upgrade)).toBe(words)
  })

  it('name the Satchel', () => {
    expect(satchelWords(['Opt', 'Sol Ring'])).toBe('Satchel: Opt, Sol Ring')
    expect(satchelWords([])).toBe('Satchel empty')
  })
})

describe('the offers', () => {
  it('say a picked card joins the Satchel', () => {
    offer({ Cards: { cards: ['Opt'] } })
    expect(screen.getByText(/joins your Satchel/)).toBeInTheDocument()
  })

  it('offer Temper at the campfire with each upgrade, and send the one taken', () => {
    const onPick = offer({ Campfire: { upgrades: UPGRADES } })
    expect(screen.getByText('Pick a new card for your Satchel.')).toBeInTheDocument()
    const temper = screen.getByRole('heading', { name: 'Temper' }).closest('[data-slot="temper"]')
    expect(temper).not.toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Atraxa has first strike' }))
    expect(onPick).toHaveBeenCalledWith({ Take: 1 })
  })

  it('leave Temper out when no upgrade fits', () => {
    offer({ Campfire: { upgrades: [] } })
    expect(screen.queryByRole('heading', { name: 'Temper' })).toBeNull()
  })

  it.each([
    [UPGRADE_PRICE, false],
    [UPGRADE_PRICE - 1, true],
  ])('sell an upgrade at the shop with %i gold (shut: %s)', (gold, shut) => {
    const onPick = offer(
      { Shop: { items: ['Draught', { Upgrade: { upgrade: UPGRADES[2]!, sold: false } }] } },
      gold,
    )
    const buy = screen.getByRole('button', { name: `Atraxa gets +2/+2 (${UPGRADE_PRICE} gold)` })
    expect(buy).toHaveProperty('disabled', shut)
    if (!shut) {
      fireEvent.click(buy)
      expect(onPick).toHaveBeenCalledWith({ Take: 1 })
    }
  })

  it('list the cards the Mirror Pool may copy, find them by name and send the copy', () => {
    const onPick = offer('Duplicate', 0, ['Cultivate', 'Opt', 'Sol Ring'])
    const list = screen.getByRole('list', { name: 'Cards to copy' })
    expect(within(list).getAllByRole('button')).toHaveLength(3)
    fireEvent.change(screen.getByRole('searchbox', { name: 'Find a card' }), { target: { value: 'sol' } })
    expect(within(list).getAllByRole('button')).toHaveLength(1)
    fireEvent.click(screen.getByRole('button', { name: 'Copy Sol Ring' }))
    expect(onPick).toHaveBeenCalledWith({ Duplicate: 'Sol Ring' })
    fireEvent.click(screen.getByRole('button', { name: 'Leave the pool' }))
    expect(onPick).toHaveBeenCalledWith('Skip')
  })

  it('name what the Mirror Pool option does before the choice', () => {
    expect(chipOf('Duplicate')).toEqual({
      kind: 'card',
      tone: 'gain',
      text: 'a copy of a card from your deck or Satchel',
    })
  })
})

it('lists each hero’s Satchel and upgrades in the run header', async () => {
  const view = delveView(
    {},
    {
      status: 'Map',
      heroes: [
        hero({ satchel: ['Opt', 'Sol Ring'], upgrades: [UPGRADES[0]!] }),
        hero({ name: 'Bo' }),
      ],
    },
  )
  await renderInRouter(
    <TooltipProvider>
      <RunHeader view={view} content={CONTENT} />
    </TooltipProvider>,
  )
  expect(screen.getByText('Satchel: Opt, Sol Ring')).toBeInTheDocument()
  expect(screen.getByText('Atraxa costs {1} less to cast')).toBeInTheDocument()
  expect(document.querySelectorAll('[data-slot="hero-gear"]')).toHaveLength(1)
})
