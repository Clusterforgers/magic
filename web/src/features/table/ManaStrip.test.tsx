/** Goal: a card shows a small strip of the mana a tap gives only on a permanent its seat may tap for mana (untapped, theirs, on the battlefield, not a creature that came this turn) and nowhere else, with the symbols of the land's mana, and the card preview lists the same symbols in words. */
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import type { ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { manaLetter } from './cardModel'
import { fixtureGame } from './dev/devGame'
import { mayTap } from './manaMade'

function open() {
  const game = fixtureGame({ seats: 2 })
  render(
    <GameContext value={game}>
      <TableOptionsContext value={{}}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return must(game.session.store.getState().view)
}
const card = (id: number): HTMLElement | null =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)
const strip = (id: number): HTMLElement | null => card(id)?.querySelector('[data-slot="mana-strip"]') ?? null
const makesMana = (object: ObjectView): boolean => object.abilities.some((ability) => 'Mana' in ability)

describe('the corner strip of a card', () => {
  it('shows on the untapped permanents the seat may tap for mana, with the mana of their ability', () => {
    const view = open()
    const mine = view.objects.filter(
      (object) => object.zone === 'Battlefield' && makesMana(object) && mayTap(object, view.viewer),
    )
    const shown = mine.filter((object) => strip(object.id) !== null)
    expect(shown.length).toBeGreaterThan(0)
    for (const object of shown) {
      const made = object.abilities.flatMap((ability) =>
        'Mana' in ability ? [manaLetter(ability.Mana.produces)] : [],
      )
      const pips = [...must(strip(object.id)).querySelectorAll('[data-pip]')].map((pip) =>
        pip.getAttribute('data-pip'),
      )
      expect(pips).toEqual(made)
    }
  })

  it("stays off a tapped permanent, another seat's, a card in a hand and a card that makes no mana", () => {
    const view = open()
    const drawn = view.objects.filter((object) => card(object.id) !== null)
    expect(drawn.length).toBeGreaterThan(10)
    for (const object of drawn) {
      const may = object.zone === 'Battlefield' && makesMana(object) && mayTap(object, view.viewer)
      if (!may) expect(strip(object.id), `${object.name} (${String(object.zone)})`).toBeNull()
    }
    expect(drawn.some((object) => object.permanent?.tapped === true && makesMana(object))).toBe(true)
  })

  it("is decoration: hidden from a screen reader, since the card's name already says what a tap adds", () => {
    const view = open()
    const object = must(
      view.objects.find(
        (o) => o.zone === 'Battlefield' && makesMana(o) && mayTap(o, view.viewer) && strip(o.id),
      ),
    )
    expect(must(strip(object.id))).toHaveAttribute('aria-hidden', 'true')
    // The card's own name already says what a tap adds, so the strip is only for the eye.
    expect(card(object.id)).toHaveAccessibleName(/add .* mana/i)
  })

  it('is listed in words by the card preview', async () => {
    const view = open()
    const object = must(
      view.objects.find(
        (o) => o.zone === 'Battlefield' && makesMana(o) && mayTap(o, view.viewer) && strip(o.id),
      ),
    )
    await userEvent.setup().hover(must(card(object.id)))
    const preview = await screen.findByTestId('card-preview')
    const line = must(preview.querySelector('[data-slot="preview-mana"]'))
    const produced = object.abilities.flatMap((ability) =>
      'Mana' in ability ? [ability.Mana.produces.toLowerCase()] : [],
    )
    expect(within(line as HTMLElement).getByRole('img')).toHaveAccessibleName(produced.join(' or '))
    expect(line).toHaveTextContent('Taps for')
  })
})
