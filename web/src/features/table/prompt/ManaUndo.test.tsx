/** Goal: at priority a source tapped for mana just now is marked legal and a press on it answers `UndoMana`, while one that cannot be untapped stays dimmed, sends nothing and says why on its foot and in its name for each refusal; on the dev table a land tapped and pressed again is untapped with an empty pool, at 2 and 4 seats. */
import { act } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, ManaUndoRefusal, ObjectId, PriorityOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { fixtureGame } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { MANA_UNDO_REASONS } from './manaUndo'

const view = buildFixture({ seats: 2 }).view
const land = must(
  view.objects.find(
    (object) => object.zone === 'Battlefield' && object.controller === 0 && object.types.includes('Land'),
  ),
).id

const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: false,
  can_pass: true,
  ...over,
})

function renderPriority(given: PriorityOptions) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state) => ({ ...state, pending: { id: 4, decision: { player: 0, kind: { Priority: given } } } }),
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  return { ...table, answer, card }
}

describe('pressing a source tapped for mana again', () => {
  it('untaps it: the card is legal and a press answers UndoMana', async () => {
    const { card, answer, user } = renderPriority(options({ undo_mana: [{ source: land }] }))
    expect(card(land)).toHaveAttribute('data-highlight', 'legal')
    expect(card(land)).toHaveAccessibleName(/press to untap .* and take its mana back/)
    await user.click(card(land))
    expect(answer.mock.calls).toEqual([[{ Priority: { UndoMana: land } }]])
  })

  it.each(Object.entries(MANA_UNDO_REASONS) as [ManaUndoRefusal, string][])(
    'refused as %s: dimmed, nothing is sent and the foot says "%s"',
    async (refusal, reason) => {
      const { card, answer, user } = renderPriority(options({ undo_mana: [{ source: land, refusal }] }))
      expect(card(land)).toHaveAttribute('data-highlight', 'dimmed')
      expect(card(land).querySelector('[data-slot="card-hint"]')?.textContent).toBe(reason)
      expect(card(land)).toHaveAccessibleName(new RegExp(`cannot untap: ${reason.toLowerCase()}`))
      await user.click(card(land))
      expect(answer).not.toHaveBeenCalled()
    },
  )
})

describe('the dev table', () => {
  it.each([2, 4])('taps a land and untaps it again on a second press at %i seats', (seats) => {
    const game = fixtureGame({ seats, prompt: 'priority-main' })
    const { store } = game.session
    const pool = (): number => must(store.getState().view?.players[0]).mana.counts.reduce((a, b) => a + b, 0)
    const kind = must(store.getState().pending).decision.kind
    const option = must(
      'Priority' in kind
        ? kind.Priority.mana_abilities.find((one) => one.produces !== 'Colorless')
        : undefined,
    )
    const id = option.source.object
    const tapped = (): boolean => store.getState().objectsById[id]?.permanent?.tapped === true
    const before = pool()
    act(() => void game.session.answer({ Priority: { ActivateMana: option.source } }))
    expect([tapped(), pool()]).toEqual([true, before + 1])
    act(() => void game.session.answer({ Priority: { UndoMana: id } }))
    expect([tapped(), pool()]).toEqual([false, before])
  })
})
