// Goal: a seat's corner draws its commanders, the top card of its graveyard and of its exile as pictures with their counts (the commander with its tax), an empty zone as a faint outline with its name, sizes the pictures by the lane card and the room its pod column leaves and keeps the chips where the least picture does not fit, opens the zone's list from a press or the keyboard, and leaves your own commanders to the hand dock.
import { screen, within } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { StateView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { NO_ROOM, PodRoomContext } from './podRoom'
import type { PodRoom } from './podRoom'
import { SeatPanel } from './SeatPanel'

// Every render of a card face is counted, to see what a batch redraws.
const probe = vi.hoisted(() => ({ faces: new Map<number, number>() }))

vi.mock('./CardFace', async (importOriginal) => {
  const original = await importOriginal<typeof import('./CardFace')>()
  const { createElement } = await import('react')
  return {
    ...original,
    CardFace(props: ComponentProps<typeof original.CardFace>) {
      probe.faces.set(props.object.id, (probe.faces.get(props.object.id) ?? 0) + 1)
      return createElement(original.CardFace, props)
    },
  }
})

afterEach(() => {
  vi.restoreAllMocks()
  resetImageMemory()
  probe.faces.clear()
})

/** A duel at the table of the dev route: Alice (you) and Bo, whose pod has a commander, five cards in the graveyard and two in exile. */
const DUEL = buildFixture({ seats: 2, board: 'duel' })

function edited(edit: (view: StateView) => void): (state: PlayState) => PlayState {
  return (state) => {
    const view = clone(must(state.view))
    edit(view)
    return { ...state, view, objectsById: indexById(view.objects) }
  }
}

interface Show {
  seat?: number
  room?: PodRoom
  edit?: (view: StateView) => void
  fixture?: ReturnType<typeof buildFixture>
}

/** A pod panel in a column of the given room; `above` is the height of the blocks over its corner. */
function show({ seat = 1, room = NO_ROOM, edit = () => undefined, fixture = DUEL }: Show = {}) {
  return renderTable({
    fixture,
    ui: (
      <PodRoomContext.Provider value={room}>
        <SeatPanel seat={seat} />
      </PodRoomContext.Provider>
    ),
    state: edited(edit),
  })
}

const pod = (name: string): HTMLElement => screen.getByRole('region', { name })
const corner = (name: string): HTMLElement =>
  must(pod(name).querySelector<HTMLElement>('[data-slot="seat-corner"]'), `${name}'s corner`)
const size = (element: HTMLElement): string => element.style.getPropertyValue('--card-w')

describe('the pictures of a corner', () => {
  it('draws the commander, the graveyard and the exile, each with its name and count', () => {
    show()
    const bo = corner('Bo')
    expect(within(bo).getByRole('list', { name: "Bo's command zone" })).toHaveTextContent('Atraxa')
    const yard = within(bo).getByRole('button', { name: 'Graveyard 5 cards, opens the list' })
    const exile = within(bo).getByRole('button', { name: 'Exile 2 cards, opens the list' })
    expect(yard.querySelector('[data-slot="zone-count"]')).toHaveTextContent('5')
    expect(exile.querySelector('[data-slot="zone-count"]')).toHaveTextContent('2')
    expect(bo).toHaveAttribute('role', 'group')
    expect(bo).toHaveAccessibleName("Bo's zones")
  })

  it('shows the top card of each zone: the last one, named in the tooltip', () => {
    const { store } = show()
    const view = must(store.getState().view)
    const yard = must(view.players[1]).graveyard.cards ?? []
    const last = must(store.getState().objectsById[must(yard.at(-1))])
    const button = within(corner('Bo')).getByRole('button', { name: /^Graveyard/ })
    expect(button).toHaveAttribute('data-top', String(last.id))
    expect(button).toHaveAttribute('title', `Graveyard 5 cards. Top card: ${last.name}`)
    expect(button).toHaveTextContent(last.name)
  })

  it('draws a card with a picture source as its picture and any other card as its text face', () => {
    show({
      edit: (view) => {
        const yard = must(view.players[1]).graveyard.cards ?? []
        const top = must(view.objects.find((object) => object.id === yard.at(-1)))
        top.identity = { oracle_id: 'yard-top' }
      },
    })
    const bo = corner('Bo')
    const yard = within(bo).getByRole('button', { name: /^Graveyard/ })
    expect(must(yard.querySelector('img')).getAttribute('src')).toBe('/api/images/oracle/yard-top/0/small')
    expect(
      within(bo)
        .getByRole('button', { name: /^Exile/ })
        .querySelector('img'),
    ).toBeNull()
  })

  it('writes the tax of a commander on its card, named in words, and the damage it took as a figure in the chips', () => {
    show()
    const bo = corner('Bo')
    const tax = must(
      bo.querySelector<HTMLElement>('[data-slot="corner-commander"] [data-slot="commander-tax"]'),
    )
    expect(tax).toHaveTextContent('+4')
    expect(tax).toHaveAccessibleName(
      'Atraxa, Praetors’ Voice: cast 2 times from the command zone, tax 4 mana',
    )
    const damage = must(
      pod('Bo').querySelector<HTMLElement>('[data-slot="pod-counts"] [data-slot="commander-damage"]'),
    )
    expect(damage).toHaveTextContent('6/21')
    expect(damage).toHaveAccessibleName(/^6 of 21 commander damage from /)
  })

  it('names the commander on each damage chip once there are several', () => {
    show({
      edit: (view) => {
        const [mine] = must(view.players[0]).commanders ?? []
        const [theirs] = must(view.players[1]).commanders ?? []
        must(view.players[1]).commander_damage = [
          { commander: must(mine).object, damage: 6 },
          { commander: must(theirs).object, damage: 3 },
        ]
      },
    })
    const chips = [...pod('Bo').querySelectorAll('[data-slot="commander-damage"]')]
    expect(chips.map((chip) => chip.textContent)).toEqual([
      expect.stringMatching(/ 6\/21$/),
      expect.stringMatching(/ 3\/21$/),
    ])
  })

  it('lays partners over each other in one slot, the head of the first showing', () => {
    show({ seat: 0, fixture: buildFixture({ seats: 2, viewer: 1 }) })
    const alice = corner('Alice')
    const list = within(alice).getByRole('list', { name: "Alice's command zone" })
    expect(within(list).getAllByRole('button')).toHaveLength(2)
    expect(size(list)).toBe('44px')
    expect(list.style.gridTemplateRows).toBe('repeat(1, 16px) 62px')
    expect(alice.children).toHaveLength(3)
  })

  it('leaves your own commanders to the hand dock: no card in the corner, the tax as a chip above it', () => {
    show({ seat: 0 })
    const alice = pod('Alice (you)')
    const zones = within(alice).getByRole('group', { name: "Alice's zones" })
    expect(within(zones).queryByRole('list', { name: /command zone/ })).toBeNull()
    expect(zones.querySelector('[data-zone="command"]')).toBeNull()
    expect(within(zones).getAllByRole('button')).toHaveLength(2)
    expect(alice.querySelector('[data-slot="pod-counts"] [data-slot="commander-tax"]')).not.toBeNull()
  })
})

describe('what a batch redraws', () => {
  it('redraws no picture of the corner when only the seat’s life changes, and the top card when it changes', () => {
    const { store, update } = show()
    const view = must(store.getState().view)
    const yard = must(view.players[1]).graveyard.cards ?? []
    const top = must(yard.at(-1))
    const exile = view.exile.cards ?? []
    expect(probe.faces.get(top)).toBeGreaterThan(0)
    probe.faces.clear()
    // The seat's player is a new object and no card is.
    update((state) => ({
      ...state,
      view: {
        ...must(state.view),
        players: must(state.view).players.map((player) =>
          player.seat === 1 ? { ...player, life: player.life - 3 } : player,
        ),
      },
    }))
    expect(probe.faces.size).toBe(0)
    update((state) => ({
      ...state,
      objectsById: { ...state.objectsById, [top]: { ...must(state.objectsById[top]), name: 'Renamed' } },
    }))
    expect([...probe.faces.keys()]).toEqual([top])
    expect(exile.length).toBeGreaterThan(0)
  })
})

describe('an empty zone', () => {
  const emptyExile = (view: StateView): void => {
    view.exile = { count: 0, cards: [] }
  }

  it('is a faint outline with the name of the zone and no picture', () => {
    show({ edit: emptyExile })
    const exile = within(corner('Bo')).getByRole('button', { name: 'Exile 0 cards' })
    expect(exile).toBeDisabled()
    expect(exile).toHaveAttribute('data-empty', 'true')
    expect(exile).toHaveTextContent('Exile')
    expect(exile.querySelector('img, [data-slot="card-picture"], [data-slot="card-face"]')).toBeNull()
    expect(exile.className).toMatch(/border-dashed/)
  })

  it('writes the whole name where the picture is wide enough and a short one where it is not', () => {
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(100)
    const emptyYard = (view: StateView): void => {
      must(view.players[0]).graveyard = { count: 0, cards: [] }
    }
    const { unmount } = show({ seat: 0, room: { height: 420, card: 154 }, edit: emptyYard })
    const wide = within(corner('Alice (you)')).getByRole('button', { name: 'Graveyard 0 cards' })
    expect(size(wide)).toBe('92px')
    expect(wide).toHaveTextContent('Graveyard')
    unmount()
    show({ seat: 0, edit: emptyYard })
    expect(
      within(corner('Alice (you)')).getByRole('button', { name: 'Graveyard 0 cards' }),
    ).toHaveTextContent('Grave')
  })

  it('draws the command zone as an outline while the commander is elsewhere in a Commander game', () => {
    show({
      edit: (view) => {
        must(view.players[1]).command = { count: 0, cards: [] }
      },
    })
    const command = within(corner('Bo')).getByRole('img', { name: 'Command zone 0 cards' })
    expect(command).toHaveAttribute('data-empty', 'true')
    expect(command).toHaveTextContent('Cmd')
  })
})

describe('opening the list', () => {
  it('opens on a press and on Enter, lists the cards most recent first and gives the focus back on Escape', async () => {
    const { user, store } = show()
    const yard = must(must(store.getState().view).players[1]).graveyard.cards ?? []
    const button = within(corner('Bo')).getByRole('button', { name: /^Graveyard 5 cards/ })
    await user.click(button)
    const dialog = await screen.findByRole('dialog', { name: /graveyard/i })
    const shown = within(within(dialog).getByRole('list')).getAllByRole('button')
    expect(shown.map((item) => Number(item.getAttribute('data-object-id')))).toEqual(yard.toReversed())
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(button).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(await screen.findByRole('dialog', { name: /graveyard/i })).toBeInTheDocument()
  })

  it('lists only the exiled cards of the seat', async () => {
    const { user } = show()
    await user.click(within(corner('Bo')).getByRole('button', { name: /^Exile 2 cards/ }))
    const dialog = await screen.findByRole('dialog', { name: /exile/i })
    expect(within(within(dialog).getByRole('list')).getAllByRole('button')).toHaveLength(2)
  })

  it('reaches every picture by the keyboard', async () => {
    const { user } = show()
    await user.tab()
    await user.tab()
    const reached = new Set<string>()
    for (let step = 0; step < 12; step += 1) {
      const focused = document.activeElement
      if (focused instanceof HTMLElement && corner('Bo').contains(focused)) {
        reached.add(
          focused.getAttribute('data-zone') ??
            focused.closest('[data-zone]')?.getAttribute('data-zone') ??
            '',
        )
      }
      await user.tab()
    }
    expect([...reached].toSorted()).toEqual(['command', 'exile', 'graveyard'])
  })
})

describe('the size of the pictures', () => {
  const above = (height: number) =>
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(height)

  it('is the least picture while the column is not measured', () => {
    show()
    const bo = corner('Bo')
    expect(bo).toHaveAttribute('data-picture', '44')
    expect(size(within(bo).getByRole('button', { name: /^Graveyard/ }))).toBe('44px')
  })

  it.each<[string, PodRoom, number]>([
    ['a column of 183 px beside cards of 120 px', { height: 183, card: 120 }, 48],
    ['a column of 232 px beside cards of 160 px', { height: 232, card: 160 }, 48],
    ['a column of 183 px beside cards of 60 px', { height: 183, card: 60 }, 44],
    ['a column not measured beside cards of 120 px', { height: null, card: 120 }, 44],
  ])('is %s', (_name, room, picture) => {
    above(74)
    show({ room })
    const bo = corner('Bo')
    expect(bo).toHaveAttribute('data-picture', String(picture))
    expect(size(within(bo).getByRole('button', { name: /^Graveyard/ }))).toBe(`${picture}px`)
  })

  it('stacks two zones in one column of large pictures beside a large board, as yours does', () => {
    above(100)
    show({ seat: 0, room: { height: 412, card: 154 } })
    const alice = corner('Alice (you)')
    expect(alice).toHaveAttribute('data-picture', '92')
    expect(alice.style.gridTemplateColumns).toBe('repeat(1, 92px)')
  })

  it('keeps the chips and the small cards of the pod where the column has no room for the least picture', () => {
    above(74)
    show({ room: { height: 100, card: 120 } })
    const bo = pod('Bo')
    expect(bo.querySelector('[data-slot="seat-corner"]')).toBeNull()
    expect(within(bo).getByRole('button', { name: 'Graveyard 5 cards, opens the list' })).toHaveTextContent(
      '5',
    )
    expect(within(bo).getByRole('button', { name: 'Exile 2 cards, opens the list' })).toHaveTextContent('2')
    expect(size(within(bo).getByRole('list', { name: "Bo's command zone" }))).toBe('44px')
    expect(bo.querySelector('[data-slot="commander-chips"] [data-slot="commander-tax"]')).not.toBeNull()
  })
})
