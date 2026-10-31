// Goal: a Room of the viewer with a locked door they may unlock now is marked legal and named "Unlock <door>", a click unlocks the one door offered or opens the chooser at the card when there are two (or an ability beside it), a line says what the door costs and answers "UnlockDoor" with the Room and the door, and a Room of an opponent, a door the offer leaves out or a Room that left the battlefield is never offered.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { indexById } from '@/play/view'
import type { PlayState } from '@/play/store'
import type {
  ActivatedView,
  Answer,
  FaceOption,
  ObjectId,
  ObjectView,
  PriorityOptions,
} from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { permanent } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'

const ROOM = 920
const THEIR_ROOM = 921
const ENTRYWAY = 'Grand Entryway'
const ROTUNDA = 'Elegant Rotunda'

const door = (face: number, name: string, cost: FaceOption['cost']): FaceOption => ({
  card: ROOM,
  face,
  name,
  ...(cost === undefined ? {} : { cost }),
  land: false,
})
const FIRST = door(0, ENTRYWAY, [{ Generic: 1 }, { Colored: 'White' }])
const SECOND = door(1, ROTUNDA, [{ Generic: 2 }, { Colored: 'White' }])

const room = (id: number, controller: number, over: Partial<ObjectView> = {}): ObjectView =>
  permanent(
    { doors: [false, false] },
    {
      id,
      owner: controller,
      controller,
      name: '',
      mana_cost: null,
      types: ['Enchantment'],
      subtypes: ['Room'],
      power: null,
      toughness: null,
      ...over,
    },
  )

const options = (unlockable: FaceOption[], over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  unlockable,
  can_act: true,
  can_pass: true,
  ...over,
})

function renderRooms(given: PriorityOptions, rooms: ObjectView[] = [room(ROOM, 0), room(THEIR_ROOM, 1)]) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state: PlayState) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, ...rooms]
      view.battlefield = {
        count: view.battlefield.count + rooms.length,
        cards: [...(view.battlefield.cards ?? []), ...rooms.map((object) => object.id)],
      }
      return {
        ...state,
        view,
        objectsById: indexById(view.objects),
        pending: { id: 4, decision: { player: 0, kind: { Priority: given } } },
      }
    },
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const items = (): HTMLElement[] =>
    within(screen.getByRole('menu', { name: /^What to do with/ })).getAllByRole('menuitem')
  return { ...table, answer, card, items }
}

describe('a Room with one locked door to unlock', () => {
  it('is marked legal and named by the door it unlocks', () => {
    const { card } = renderRooms(options([FIRST]))
    expect(card(ROOM)).toHaveAttribute('data-highlight', 'legal')
    expect(card(ROOM).getAttribute('aria-label')).toContain('Unlock Grand Entryway')
  })

  it('unlocks the door on a click, without a chooser', async () => {
    const { card, answer, user } = renderRooms(options([FIRST]))
    await user.click(card(ROOM))
    expect(answer.mock.calls).toEqual([[{ Priority: { UnlockDoor: [ROOM, 0] } }]])
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('is not offered to the opponent’s Room', async () => {
    const { card, answer, user } = renderRooms(options([FIRST]))
    expect(card(THEIR_ROOM)).not.toHaveAttribute('data-highlight', 'legal')
    await user.click(card(THEIR_ROOM))
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('a Room with two locked doors', () => {
  it('asks which door in a chooser at the card, each line "Unlock <door>" with its cost', async () => {
    const { card, items, user } = renderRooms(options([FIRST, SECOND]))
    expect(card(ROOM).getAttribute('aria-label')).toContain('Unlock one of 2 doors')
    await user.click(card(ROOM))
    expect(items().map((item) => item.getAttribute('aria-label'))).toEqual([
      `Unlock ${ENTRYWAY}`,
      `Unlock ${ROTUNDA}`,
    ])
    expect(items().map((item) => item.textContent)).toEqual([
      `Unlock ${ENTRYWAY}Pay {1}{W}`,
      `Unlock ${ROTUNDA}Pay {2}{W}`,
    ])
  })

  it.each<[string, number, number]>([
    ['the first door', 0, 0],
    ['the second door', 1, 1],
  ])('unlocks %s when its line is picked', async (_case, line, doorAt) => {
    const { card, items, answer, user } = renderRooms(options([FIRST, SECOND]))
    await user.click(card(ROOM))
    await user.click(must(items()[line]))
    expect(answer.mock.calls).toEqual([[{ Priority: { UnlockDoor: [ROOM, doorAt] } }]])
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('closes the chooser on Escape without answering', async () => {
    const { card, answer, user } = renderRooms(options([FIRST, SECOND]))
    await user.click(card(ROOM))
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menu')).toBeNull()
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('a Room with an ability and a door to unlock', () => {
  const ability = { object: ROOM, ability: 0 }
  const worded: ActivatedView = { ability, cost: '{T}', effect: 'Scry 1.' }

  it('lists the ability, then the door', async () => {
    const { card, items, user } = renderRooms(options([FIRST], { abilities: [ability] }), [
      room(ROOM, 0, { activated: [worded] }),
    ])
    expect(card(ROOM).getAttribute('aria-label')).toContain('Choose one of 2 plays')
    await user.click(card(ROOM))
    expect(items().map((item) => item.getAttribute('aria-label'))).toEqual([
      'Activate tap: Scry 1.',
      `Unlock ${ENTRYWAY}`,
    ])
  })
})

describe('doors the offer does not name', () => {
  it.each<[string, PriorityOptions, ObjectView[]]>([
    ['none to unlock', options([]), [room(ROOM, 0)]],
    ['a Room that is not on the battlefield', options([FIRST]), [room(ROOM, 0, { zone: { Graveyard: 0 } })]],
    ['a Room of another seat', options([FIRST]), [room(ROOM, 1)]],
  ])('marks nothing for %s', (_case, given, rooms) => {
    const { card } = renderRooms(given, rooms)
    expect(card(ROOM)).not.toHaveAttribute('data-highlight', 'legal')
  })
})
