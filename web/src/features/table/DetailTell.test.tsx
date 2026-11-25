// Goal: at two and four seats, on a desktop dock, a short dock and a phone, the refusals the engine details say why in one plain line where the player looks: a press on an Aura in hand with nothing to enchant names the host it needs, a press on a graveyard card whose unearth is short of mana names the mana, an X ability names the target no payable X has in the permanent's preview words, a press on an X spell in hand names the target no payable X has, an ability paid only with the chosen color names that mana, one short at its least X names the X, and an activation taken back for its mana says so in the centre strip.
import { render } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import type { Viewport } from '@/features/table/layout'
import { must } from '@/test-utils/batches'
import { useBlockedTell } from './blockedTell'
import { fixtureGame } from './dev/devGame'
import {
  DETAIL_AURA,
  DETAIL_CHOSEN,
  DETAIL_LEAST_X,
  DETAIL_UNEARTH,
  DETAIL_X,
  DETAIL_X_SPELL,
} from './dev/detailScene'
import { inactiveNotes } from './handReason'

afterEach(() => useBlockedTell.getState().hush())

const LAYOUTS: readonly [string, Viewport][] = [
  ['a desktop dock', { width: 1400, height: 900 }],
  ['a short dock', { width: 1280, height: 640 }],
  ['a phone', { width: 390, height: 844 }],
]

function scene(seats: number, viewport: Viewport) {
  const game = fixtureGame({ seats, prompt: 'priority-detail' })
  render(
    <GameContext value={game}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  const state = game.session.store.getState()
  const idOf = (name: string): number =>
    must(Object.values(state.objectsById).find((o) => o.name === name)).id
  return { game, state, idOf, user: userEvent.setup() }
}

const tell = (): HTMLElement | null => document.querySelector<HTMLElement>('[data-slot="blocked-tell"]')
const cardButton = (id: number): HTMLElement | null =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)

describe.each([2, 4])('detailed refusals at %i seats', (seats) => {
  describe.each(LAYOUTS)('on %s', (_layout, viewport) => {
    it('says the host an Aura in hand lacks when it is pressed', async () => {
      const { idOf, user } = scene(seats, viewport)
      await user.click(must(cardButton(idOf(DETAIL_AURA))))
      expect(must(tell())).toHaveTextContent(`${DETAIL_AURA} needs a Wall on the battlefield to enchant.`)
    })

    it('says why the unearth card in the graveyard cannot be used when it is pressed in the graveyard', async () => {
      const { state, idOf, user } = scene(seats, viewport)
      const kind = must(state.pending).decision.kind
      if (!('Priority' in kind)) throw new Error('a priority decision')
      const zombie = idOf(DETAIL_UNEARTH)
      const note = must(inactiveNotes(state.objectsById, kind.Priority).get(zombie))
      expect(note.hint).toBe('Needs mana')
      const line = 'Needs {B}: you have 0 mana available. Nothing you have makes {B}.'
      expect(note.line).toBe(line)
      // The viewer's graveyard opens from its zone button; the card pressed in the list says why.
      for (const button of document.querySelectorAll<HTMLElement>('button[data-zone="graveyard"]')) {
        if (cardButton(zombie) !== null) break
        if (!button.hasAttribute('disabled')) await user.click(button)
      }
      await user.click(must(cardButton(zombie)))
      expect(must(tell())).toHaveTextContent(line)
    })

    it('says which target no payable X of the X ability has', () => {
      const { state, idOf } = scene(seats, viewport)
      const kind = must(state.pending).decision.kind
      if (!('Priority' in kind)) throw new Error('a priority decision')
      const note = must(inactiveNotes(state.objectsById, kind.Priority).get(idOf(DETAIL_X)))
      expect(note.hint).toBeUndefined()
      expect(note.line).toBe(
        `${DETAIL_X} has no target for any X you can pay (X from 0 to 2): it needs an artifact or creature card with mana value X in an opponent's graveyard.`,
      )
    })

    it('says which target no payable X of the X spell in hand has when it is pressed', async () => {
      const { idOf, user } = scene(seats, viewport)
      await user.click(must(cardButton(idOf(DETAIL_X_SPELL))))
      expect(must(tell())).toHaveTextContent(
        `${DETAIL_X_SPELL} has no target for any X you can pay (X from 0 to 2): it needs a creature with mana value X on the battlefield.`,
      )
    })

    it('says the chosen color an ability is paid with and the least X another lacks', () => {
      const { state, idOf } = scene(seats, viewport)
      const kind = must(state.pending).decision.kind
      if (!('Priority' in kind)) throw new Error('a priority decision')
      const notes = inactiveNotes(state.objectsById, kind.Priority)
      const throne = must(notes.get(idOf(DETAIL_CHOSEN)))
      expect(throne.hint).toBeUndefined()
      expect(throne.line).toBe(
        'Only red mana can be spent to activate it: needs {R}{R}{R}, and you have 1 red mana available.',
      )
      const rasputin = must(notes.get(idOf(DETAIL_LEAST_X)))
      expect(rasputin.hint).toBeUndefined()
      expect(rasputin.line).toBe("X must be at least 1, and the rest of its cost can't be paid for that.")
    })

    it('says an activation was taken back for its mana in the centre strip', () => {
      scene(seats, viewport)
      const note = must(document.querySelector('[data-slot="rewound"]'))
      expect(note).toHaveTextContent(
        `${DETAIL_X} could not be activated. Needs {3}{B}: you have 2 mana available.`,
      )
    })
  })
})
