// Goal: a seat panel shows a player's name, life, mana pool, library and hand counts, graveyard, exile and command zone, marks the active turn and priority, and says plainly when the player is eliminated, which mulligan step they are on, and where commander damage and tax will go, and keeps to one line (name, life, counts, zones) for a column of boards.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { LossReason, MulliganStatus, StateView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { SeatPanel } from './SeatPanel'

/** The fixture state with its view changed by `edit`. */
function edited(edit: (view: StateView) => void): (state: PlayState) => PlayState {
  return (state) => {
    const view = clone(must(state.view))
    edit(view)
    return { ...state, view, objectsById: indexById(view.objects) }
  }
}

const panel = (name: string): HTMLElement => screen.getByRole('region', { name })
const show = (seat: number, edit: (view: StateView) => void = () => undefined) =>
  renderTable({ seats: 4, ui: <SeatPanel seat={seat} size="normal" />, state: edited(edit) })

describe('the basics', () => {
  it('names the seat and shows its life, library and hand counts and an empty mana pool', () => {
    show(2)
    const cy = panel('Cy')
    expect(within(cy).getByRole('heading', { name: 'Cy' })).toBeInTheDocument()
    expect(cy).toHaveTextContent('Life36')
    expect(cy).toHaveTextContent('Library 46')
    expect(cy).toHaveTextContent('Hand 7')
    expect(within(cy).getByRole('group', { name: 'Mana pool' })).toHaveTextContent('empty')
  })

  it('carries data-seat for the decision layer', () => {
    show(3)
    expect(panel('Dee')).toHaveAttribute('data-seat', '3')
  })

  it('marks this seat "you", the active turn and priority', () => {
    show(0)
    const alice = panel('Alice (you)')
    expect(alice).toHaveAttribute('data-you', 'true')
    expect(alice).toHaveTextContent('you')
    expect(alice).toHaveTextContent('Has priority')
    expect(alice).not.toHaveTextContent('Active turn')
  })

  it('marks the player whose turn it is', () => {
    show(1)
    expect(panel('Bo')).toHaveAttribute('data-active', 'true')
    expect(panel('Bo')).toHaveTextContent('Active turn')
  })

  it('falls back to "Seat n" while the server has not named the seat', () => {
    renderTable({
      seats: 4,
      ui: <SeatPanel seat={2} size="normal" />,
      lookup: { game: null, missing: false, failed: false },
    })
    expect(within(panel('Seat 3')).getByRole('heading', { name: 'Seat 3' })).toBeInTheDocument()
  })

  it('draws nothing for a seat that is not at the table', () => {
    const { container } = show(9)
    expect(container.querySelector('[data-seat]')).toBeNull()
  })
})

describe('mana pool', () => {
  it('lists each kind of mana held, by count, in words for a screen reader', () => {
    show(0, (view) => {
      must(view.players[0]).mana = { counts: [1, 0, 0, 3, 0, 2] }
    })
    const pool = within(panel('Alice (you)')).getByRole('group', { name: 'Mana pool' })
    expect(pool).toHaveTextContent('1 white')
    expect(pool).toHaveTextContent('3 red')
    expect(pool).toHaveTextContent('2 colorless')
    expect(pool).not.toHaveTextContent('empty')
    expect(pool).not.toHaveTextContent('blue')
  })
})

describe('hands', () => {
  it('shows the viewer a strip of card faces', () => {
    show(0)
    const hand = within(panel('Alice (you)')).getByRole('list', { name: "Alice's hand" })
    expect(within(hand).getAllByRole('button')).toHaveLength(7)
  })

  it('shows an opponent only card backs and the count', () => {
    show(1)
    const bo = panel('Bo')
    expect(within(bo).queryByRole('list', { name: "Bo's hand" })).toBeNull()
    expect(bo.querySelector('[data-slot="hand-backs"]')).not.toBeNull()
    expect(bo).toHaveTextContent('Hand 6')
  })

  it('says when the viewer has no cards in hand', () => {
    show(0, (view) => {
      must(view.players[0]).hand = { count: 0, cards: [] }
    })
    expect(panel('Alice (you)')).toHaveTextContent('Your hand is empty.')
  })
})

describe('eliminated seats', () => {
  it.each<[LossReason, string]>([
    ['ZeroLife', 'Eliminated: ran out of life'],
    ['EmptyLibrary', 'Eliminated: could not draw from an empty library'],
    ['Conceded', 'Eliminated: conceded'],
    [{ Format: 'commander damage' }, 'Eliminated: lost: commander damage'],
  ])('says why a player is out: %j', (reason, text) => {
    show(2, (view) => {
      must(view.players[2]).lost = reason
    })
    const cy = panel('Cy')
    expect(cy).toHaveTextContent(text)
    expect(cy).toHaveAttribute('data-eliminated', 'true')
  })

  it('shows no elimination for a player still in', () => {
    show(2)
    expect(panel('Cy')).not.toHaveAttribute('data-eliminated')
    expect(panel('Cy')).not.toHaveTextContent('Eliminated')
  })
})

describe('mulligans', () => {
  it.each<[MulliganStatus, number, string]>([
    ['NeedsHand', 0, 'Mulligan: Waiting for a new hand'],
    ['Deciding', 1, 'Mulligan: Deciding whether to keep · 1 mulligan'],
    ['Mulliganing', 0, 'Mulligan: Mulligan taken, waiting for the table'],
    [{ Bottoming: 2 }, 2, 'Mulligan: Putting 2 cards on the bottom · 2 mulligans'],
    ['Kept', 0, 'Mulligan: Kept their hand'],
  ])('shows progress %j during the mulligan stage', (mulligan, mulligans, text) => {
    show(1, (view) => {
      view.stage = 'Mulligan'
      must(view.players[1]).mulligan = mulligan
      must(view.players[1]).mulligans = mulligans
    })
    expect(panel('Bo')).toHaveTextContent(text)
  })

  it('shows no mulligan line once turns are being played', () => {
    show(1, (view) => {
      must(view.players[1]).mulligan = 'Deciding'
    })
    expect(panel('Bo').querySelector('[data-slot="mulligan"]')).toBeNull()
  })
})

describe('commander slot', () => {
  const SLOT = '[data-slot="commander-stats"]'

  it('is always there in a Commander game, with the commander in the command zone', () => {
    show(1)
    const bo = panel('Bo')
    expect(bo.querySelector(SLOT)).toHaveTextContent('Commander damage and tax: not tracked yet')
    const zone = within(bo).getByRole('list', { name: "Bo's command zone" })
    expect(within(zone).getAllByRole('button')).toHaveLength(1)
    expect(zone).toHaveTextContent('Atraxa')
  })

  it('is left out of a game with no commanders', () => {
    show(1, (view) => {
      view.format = 'Normal'
      must(view.players[1]).command = { count: 0, cards: [] }
    })
    expect(panel('Bo').querySelector(SLOT)).toBeNull()
    expect(within(panel('Bo')).queryByRole('list', { name: "Bo's command zone" })).toBeNull()
  })

  it('shows up for a command zone card even when the format is not named Commander', () => {
    show(1, (view) => {
      view.format = 'Normal'
    })
    expect(panel('Bo').querySelector(SLOT)).not.toBeNull()
  })
})

describe('graveyard and exile', () => {
  it('shows the top card of the graveyard (the last one) and its count', () => {
    const { store } = show(1)
    const bo = panel('Bo')
    const top = must(must(store.getState().view).players[1]?.graveyard.cards?.at(-1))
    expect(bo.querySelector(`[data-object-id="${top}"]`)).not.toBeNull()
    expect(within(bo).getByRole('button', { name: /^Graveyard 5/ })).toBeEnabled()
  })

  it('lists every card of the graveyard, most recent first, in a dialog', async () => {
    const { user, store } = show(1)
    const ids = must(must(store.getState().view).players[1]).graveyard.cards ?? []
    await user.click(within(panel('Bo')).getByRole('button', { name: /^Graveyard 5/ }))
    const dialog = await screen.findByRole('dialog', { name: /graveyard/i })
    expect(dialog).toHaveTextContent('5 cards, most recent first.')
    const shown = within(within(dialog).getByRole('list')).getAllByRole('button')
    expect(shown.map((item) => Number(item.getAttribute('data-object-id')))).toEqual(ids.toReversed())
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('has a disabled graveyard button when the graveyard is empty', () => {
    show(1, (view) => {
      must(view.players[1]).graveyard = { count: 0, cards: [] }
    })
    expect(within(panel('Bo')).getByRole('button', { name: /^Graveyard 0/ })).toBeDisabled()
  })

  it('counts only the exiled cards the seat owns', () => {
    renderTable({
      seats: 4,
      ui: (
        <>
          <SeatPanel seat={1} size="normal" />
          <SeatPanel seat={3} size="normal" />
        </>
      ),
      state: edited((view) => {
        const extra = [
          card({ id: 7001, owner: 1, zone: 'Exile', permanent: null, name: 'Exiled A' }),
          card({ id: 7002, owner: 1, zone: 'Exile', permanent: null, name: 'Exiled B' }),
          card({ id: 7003, owner: 3, controller: 3, zone: 'Exile', permanent: null, name: 'Exiled C' }),
        ]
        view.objects = [...view.objects, ...extra]
        view.exile = { count: view.exile.count + 3, cards: [...(view.exile.cards ?? []), 7001, 7002, 7003] }
      }),
    })
    expect(within(panel('Bo')).getByRole('button', { name: /^Exile 3/ })).toBeEnabled()
    expect(within(panel('Dee')).getByRole('button', { name: /^Exile 1/ })).toBeEnabled()
  })

  it('lists the exiled cards of one seat in a dialog, and only those', async () => {
    const { user } = show(1, (view) => {
      view.objects = [
        ...view.objects,
        card({ id: 7001, owner: 1, zone: 'Exile', permanent: null, name: 'Exiled A' }),
        card({ id: 7003, owner: 3, controller: 3, zone: 'Exile', permanent: null, name: 'Exiled C' }),
      ]
      view.exile = { count: view.exile.count + 2, cards: [...(view.exile.cards ?? []), 7001, 7003] }
    })
    await user.click(within(panel('Bo')).getByRole('button', { name: /^Exile 2/ }))
    const dialog = await screen.findByRole('dialog', { name: /exile/i })
    expect(dialog).toHaveTextContent('Exiled A')
    expect(dialog).not.toHaveTextContent('Exiled C')
  })
})

describe('the one-line variant', () => {
  const line = (seat: number, edit: (view: StateView) => void = () => undefined) =>
    renderTable({ seats: 4, ui: <SeatPanel seat={seat} size="normal" variant="line" />, state: edited(edit) })

  it('keeps the root of a panel: its name, its marks for the decision layer and the data of the seat', () => {
    line(1)
    const bo = panel('Bo')
    expect(bo).toHaveAttribute('data-seat', '1')
    expect(bo).toHaveAttribute('data-variant', 'line')
    expect(bo).toHaveAttribute('data-active', 'true')
    expect(within(bo).getByRole('heading', { name: 'Bo' })).toBeInTheDocument()
  })

  it('says life, hand and library in one line, with the zones as small buttons', () => {
    line(2)
    const cy = panel('Cy')
    expect(cy).toHaveTextContent('Life 36')
    expect(cy).toHaveTextContent('Hand 7')
    expect(cy).toHaveTextContent('Library 46')
    expect(within(cy).getByRole('button', { name: /^Graveyard 5/ })).toBeEnabled()
    expect(within(cy).getByRole('button', { name: /^Exile 1/ })).toBeEnabled()
    // No command zone, mana pool or commander note: those are for the whole panel.
    expect(cy.querySelector('[data-slot="commander-stats"]')).toBeNull()
    expect(within(cy).queryByRole('group', { name: 'Mana pool' })).toBeNull()
  })

  it('marks you, the active turn and priority as small badges', () => {
    line(0)
    expect(panel('Alice (you)')).toHaveTextContent('you')
    expect(panel('Alice (you)')).toHaveTextContent('Has priority')
  })

  it('marks the active turn', () => {
    line(1)
    expect(panel('Bo')).toHaveTextContent('Active turn')
  })

  it('shows the mana pool only while there is mana in it', () => {
    line(0)
    expect(within(panel('Alice (you)')).getByRole('group', { name: 'Mana pool' })).toBeInTheDocument()
  })

  it('shows red life of 5 or less, the elimination and the mulligan step', () => {
    line(3, (view) => {
      must(view.players[3]).life = 4
      must(view.players[3]).lost = 'ZeroLife'
      view.stage = 'Mulligan'
    })
    const dee = panel('Dee')
    expect(dee).toHaveAttribute('data-eliminated', 'true')
    expect(dee).toHaveTextContent('Eliminated: ')
    expect(dee.querySelector('.text-destructive.font-bold')).toHaveTextContent('4')
    expect(dee.querySelector('[data-slot="mulligan"]')).toHaveTextContent('Mulligan: ')
  })

  it('has the panel of the viewer light up on their own turn, like the whole panel', () => {
    renderTable({
      fixture: buildFixture({ seats: 4, active: 0, permanents: 2 }),
      ui: <SeatPanel seat={0} size="normal" variant="line" />,
    })
    expect(panel('Alice (you)')).toHaveClass('your-turn-cue')
  })
})

describe('the viewer’s hand', () => {
  it('is in the panel by default, and left to the hand dock when the table says so', () => {
    const { unmount } = renderTable({ seats: 2, ui: <SeatPanel seat={0} size="normal" /> })
    expect(within(panel('Alice (you)')).getByRole('list', { name: "Alice's hand" })).toBeInTheDocument()
    unmount()
    renderTable({ seats: 2, ui: <SeatPanel seat={0} size="normal" hand={false} /> })
    expect(within(panel('Alice (you)')).queryByRole('list', { name: "Alice's hand" })).toBeNull()
  })

  it('leaves the card backs of another seat alone either way', () => {
    renderTable({ seats: 2, ui: <SeatPanel seat={1} size="normal" hand={false} /> })
    expect(panel('Bo').querySelector('[data-slot="hand-backs"]')).not.toBeNull()
  })
})
