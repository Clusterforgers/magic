/** Goal: a mana ability that does more than add mana (`PayOptions.announced`, Vexing Puzzlebox's roll) is offered in the PayMana prompt as a button and a legal card, each sending one Activate answer for its ability; a payment without one offers neither. */
import { fireEvent, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { AbilityRef, Answer, ObjectId, PayOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { cost, poolOf, source } from '@/test-utils/mana'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'

const view = buildFixture({ seats: 2 }).view
const mine = view.objects.filter((object) => object.zone === 'Battlefield' && object.controller === 0)
const land = must(mine.find((object) => object.types.includes('Land') && object.permanent?.tapped === false))
const box = must(mine.find((object) => !object.types.includes('Land')))
const spell = must(view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === 0))

const options = (announced: AbilityRef[]): PayOptions => ({
  spell: spell.id,
  cost: cost('1G'),
  pool: poolOf(''),
  sources: [source(land.id, 'G', 0)],
  auto_available: true,
  can_cancel: true,
  life: 20,
  max_life: 0,
  ...(announced.length === 0 ? {} : { announced }),
})

function renderPay(given: PayOptions) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state) => ({ ...state, pending: { id: 4, decision: { player: 0, kind: { PayMana: given } } } }),
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  return { answer, card }
}

describe('an announced mana ability while paying', () => {
  it.each([0, 1, 2])('is a button that activates ability %i of its source', (ability) => {
    const ref: AbilityRef = { object: box.id, ability }
    const { answer } = renderPay(options([ref]))
    fireEvent.click(screen.getByRole('button', { name: new RegExp(`^Activate ${box.name}`) }))
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer).toHaveBeenCalledWith({ PayMana: { Activate: ref } })
  })

  it('marks its source as legal, and a click on the card activates it', () => {
    const ref: AbilityRef = { object: box.id, ability: 1 }
    const { answer, card } = renderPay(options([ref]))
    expect(card(box.id)).toHaveAttribute('data-highlight', 'legal')
    fireEvent.click(card(box.id))
    expect(answer).toHaveBeenCalledWith({ PayMana: { Activate: ref } })
  })

  it('offers no button and does not mark the card legal without one', () => {
    const { card } = renderPay(options([]))
    expect(document.querySelector('[data-slot="pay-activate"]')).toBeNull()
    expect(card(box.id)).not.toHaveAttribute('data-highlight', 'legal')
  })
})
