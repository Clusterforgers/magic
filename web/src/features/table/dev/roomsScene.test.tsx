/** Goal: `?prompt=priority-rooms` hands the viewer, at every seat count, a Priority decision the wire parser accepts that offers each locked door of its own Rooms by name and cost, and the table draws the four Rooms with their door badges (both locked, one unlocked, both unlocked, a rival's), asks which door in a chooser, clears the prompt once one is unlocked, and previews a Room with both halves and their state. */
import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageChoice, setImageChoice } from '@/features/cards/imageSetting'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { CardsApiContext } from '@/features/table/cardsContext'
import { setDetailFetcher } from '@/features/table/printed'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './devGame'
import { roomDetail, roomsApi } from './roomsScene'

afterEach(() => resetImageChoice())

const sceneOf = (seats: number) => {
  const game = fixtureGame({ seats, prompt: 'priority-rooms', turn: 'you' })
  const { pending, view } = game.session.store.getState()
  const { decision, id } = must(pending)
  if (!('Priority' in decision.kind)) throw new Error('not a priority decision')
  return { game, view: must(view), options: decision.kind.Priority, decision, id }
}

async function drawn(seats = 2) {
  setImageChoice('text')
  setDetailFetcher(roomDetail)
  const scene = sceneOf(seats)
  render(
    <GameContext value={scene.game}>
      <CardsApiContext value={roomsApi}>
        <TableOptionsContext value={{}}>
          <GameTable />
        </TableOptionsContext>
      </CardsApiContext>
    </GameContext>,
  )
  await act(async () => {
    await Promise.resolve()
  })
  const card = (id: number): HTMLElement =>
    must(document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  return { ...scene, card, user: userEvent.setup() }
}

describe('?prompt=priority-rooms', () => {
  it.each([2, 4, 12])('offers each locked door of the viewer’s own Rooms at %i seats', (seats) => {
    const { view, options, decision, id } = sceneOf(seats)
    const offered = must(options.unlockable)
    expect(offered.map((door) => [door.name, door.face])).toEqual([
      ['Surgical Suite', 0],
      ['Hospital Room', 1],
      ['Locker Room', 1],
    ])
    for (const door of offered) {
      const room = must(view.objects.find((o) => o.id === door.card))
      expect(room.controller).toBe(0)
      expect(room.permanent?.doors?.[door.face]).toBe(false)
    }
    expect(parseMessage({ type: 'Decision', id, decision }).ok).toBe(true)
  })

  it('puts four Rooms on the table: the viewer’s three and a rival’s', () => {
    const { view } = sceneOf(2)
    const rooms = view.objects.filter((object) => object.permanent?.doors !== undefined)
    expect(rooms.map((room) => [room.controller, room.permanent?.doors, room.name])).toEqual([
      [0, [false, false], ''],
      [0, [true, false], 'Bottomless Pool'],
      [0, [true, true], 'Grand Entryway // Elegant Rotunda'],
      [1, [false, true], 'Forgotten Cellar'],
    ])
  })

  it('draws a badge of two doors on each Room, named by the card’s halves', async () => {
    await drawn()
    const badges = [...document.querySelectorAll('[data-slot="card-door"]')]
      .map((mark) => `${mark.getAttribute('data-unlocked')}:${mark.textContent}`)
      .sort()
    expect(badges).toEqual(
      [
        'false:Surgical Suite, locked',
        'false:Hospital Room, locked',
        'true:Bottomless Pool, unlocked',
        'false:Locker Room, locked',
        'true:Grand Entryway, unlocked',
        'true:Elegant Rotunda, unlocked',
        'false:Walk-In Closet, locked',
        'true:Forgotten Cellar, unlocked',
      ].sort(),
    )
  })

  it('asks which door in a chooser and clears the prompt once one is unlocked', async () => {
    const { card, options, game, user } = await drawn()
    const suite = must(options.unlockable)[0]?.card ?? 0
    expect(card(suite)).toHaveAttribute('data-highlight', 'legal')
    await user.click(card(suite))
    const menu = screen.getByRole('menu', { name: 'What to do with Surgical Suite // Hospital Room' })
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.getAttribute('aria-label')),
    ).toEqual(['Unlock Surgical Suite', 'Unlock Hospital Room'])
    await user.click(within(menu).getByRole('menuitem', { name: 'Unlock Hospital Room' }))
    expect(game.session.store.getState().pending).toBeNull()
  })

  it('previews a Room with both halves, their state and their rules text', async () => {
    const { card, view, user } = await drawn()
    const pool = must(view.objects.find((o) => o.name === 'Bottomless Pool'))
    await user.hover(card(pool.id))
    const preview = screen.getByTestId('card-preview')
    const doors = [...preview.querySelectorAll('[data-slot="preview-doors"] > li')]
    expect(doors.map((door) => door.querySelector('p')?.textContent)).toEqual([
      'Bottomless Pool unlocked',
      'Locker Room locked',
    ])
    await act(async () => {
      await Promise.resolve()
    })
    expect(
      within(preview).getByText(/^Whenever one or more creatures you control deal combat damage/),
    ).toBeInTheDocument()
  })
})
