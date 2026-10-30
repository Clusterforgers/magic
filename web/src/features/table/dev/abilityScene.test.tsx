/** Goal: `?prompt=priority-abilities` hands the viewer, at every seat count, a Priority decision the wire parser accepts whose abilities are worded on the viewer's own creatures, hand spells, graveyard card and commander, and the table draws them legal, asks which of two (a hand spell offers its cast beside its cycling), marks the graveyard and the exile that hold a card to play or use, and clears the prompt once one is activated. */
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './devGame'

const sceneOf = (seats: number) => {
  const game = fixtureGame({ seats, prompt: 'priority-abilities' })
  const { pending, view } = game.session.store.getState()
  const { decision, id } = must(pending)
  if (!('Priority' in decision.kind)) throw new Error('not a priority decision')
  return { game, view: must(view), options: decision.kind.Priority, decision, id }
}

describe('?prompt=priority-abilities', () => {
  it.each([2, 4, 12])('offers worded abilities of the viewer’s own cards at %i seats', (seats) => {
    const { view, options, decision, id } = sceneOf(seats)
    const offered = must(options.abilities)
    expect(offered).toHaveLength(7)
    for (const ability of offered) {
      const object = must(view.objects.find((o) => o.id === ability.object))
      expect(object.activated?.some((entry) => entry.ability?.ability === ability.ability)).toBe(true)
      const zone = typeof object.zone === 'object' ? Object.keys(object.zone)[0] : object.zone
      expect(['Battlefield', 'Hand', 'Graveyard', 'Command']).toContain(zone)
      expect(object.controller).toBe(0)
    }
    expect(parseMessage({ type: 'Decision', id, decision }).ok).toBe(true)
  })

  it('is drawn legal on the table, asks which of two abilities and clears when one is activated', async () => {
    const { game, options, view } = sceneOf(2)
    const count = (id: number): number => must(options.abilities).filter((a) => a.object === id).length
    const onBattlefield = must(options.abilities).filter(
      (a) => must(view.objects.find((o) => o.id === a.object)).zone === 'Battlefield',
    )
    const single = must(onBattlefield.find((a) => count(a.object) === 1)).object
    const double = must(onBattlefield.find((a) => count(a.object) === 2)).object
    render(
      <GameContext value={game}>
        <TableOptionsContext value={{}}>
          <GameTable />
        </TableOptionsContext>
      </GameContext>,
    )
    const user = userEvent.setup()
    const card = (id: number): HTMLElement =>
      must(document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
    expect(card(single)).toHaveAttribute('data-highlight', 'legal')
    expect(card(double)).toHaveAttribute('data-highlight', 'legal')
    await user.click(card(double))
    expect(screen.getAllByRole('menuitem')).toHaveLength(2)
    await user.keyboard('{Escape}')
    expect(game.session.store.getState().pending).not.toBeNull()
    await user.click(card(single))
    expect(game.session.store.getState().pending).toBeNull()
  })

  it('offers a hand spell its cast beside its cycling, and a spell it cannot cast its cycling alone', async () => {
    const { game, options, view } = sceneOf(2)
    const inHand = must(options.abilities).filter((a) => {
      const zone = must(view.objects.find((o) => o.id === a.object)).zone
      return typeof zone === 'object' && 'Hand' in zone
    })
    const [castable, stuck] = [
      must(inHand.find((a) => options.castable.includes(a.object))),
      must(inHand.find((a) => !options.castable.includes(a.object))),
    ]
    render(
      <GameContext value={game}>
        <TableOptionsContext value={{}}>
          <GameTable />
        </TableOptionsContext>
      </GameContext>,
    )
    const user = userEvent.setup()
    const card = (id: number): HTMLElement =>
      must(document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
    expect(card(stuck.object)).toHaveAttribute('data-highlight', 'legal')
    expect(card(stuck.object).getAttribute('aria-label')).toMatch(/Activate Cycling 2, not castable/)
    await user.click(card(castable.object))
    expect(screen.getAllByRole('menuitem').map((item) => item.dataset['play'])).toEqual([
      'cast',
      expect.stringMatching(/^ability-/),
    ])
    await user.click(screen.getByRole('menuitem', { name: 'Activate Cycling 2' }))
    expect(game.session.store.getState().pending).toBeNull()
  })

  it('marks the graveyard and the exile chips that hold a card to play or use', () => {
    const { game, view } = sceneOf(2)
    render(
      <GameContext value={game}>
        <TableOptionsContext value={{}}>
          <GameTable />
        </TableOptionsContext>
      </GameContext>,
    )
    const me = must(view.players[view.viewer])
    const zone = (name: string): HTMLElement[] =>
      [...document.querySelectorAll<HTMLElement>(`[data-zone="${name}"][data-legal]`)].filter(
        (chip) => chip.getAttribute('aria-label')?.includes('opens the list') === true,
      )
    expect(me.graveyard.count).toBeGreaterThan(0)
    expect(zone('graveyard').length).toBeGreaterThan(0)
    expect(zone('exile').length).toBeGreaterThan(0)
    expect(zone('graveyard')[0]).toHaveAccessibleName(/1 can be played or used, opens the list$/)
    expect(zone('command')).toHaveLength(0)
  })
})
