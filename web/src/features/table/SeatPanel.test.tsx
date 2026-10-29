// Goal: a seat pod, as a panel for a 168 px column and as one line, names its seat with a dot, shows life as the largest figure, draws mana only while some floats, counts hand, library, graveyard and exile in chips that each have an accessible name, shows the command zone only when it holds a card, carries the turn marks as `data-state`, says plainly when the player is out or which mulligan step they are on, and never draws a placeholder for what the views do not carry.
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

type Variant = 'panel' | 'line'
const VARIANTS: Variant[] = ['panel', 'line']

const panel = (name: string): HTMLElement => screen.getByRole('region', { name })
const show = (seat: number, edit: (view: StateView) => void = () => undefined, variant: Variant = 'panel') =>
  renderTable({ seats: 4, ui: <SeatPanel seat={seat} variant={variant} />, state: edited(edit) })
const chip = (root: HTMLElement, name: string): HTMLElement => within(root).getByRole('img', { name })

describe.each(VARIANTS)('the %s pod', (variant) => {
  it('names the seat with a dot, shows life, and counts the four zones in chips with names', () => {
    show(2, undefined, variant)
    const cy = panel('Cy')
    expect(within(cy).getByRole('heading', { name: 'Cy' })).toBeInTheDocument()
    expect(cy.querySelector('[data-slot="seat-dot"]')).toHaveAttribute('aria-hidden', 'true')
    expect(cy.querySelector('[data-slot="life"]')).toHaveTextContent('Life 36')
    expect(chip(cy, 'Hand 7 cards')).toHaveTextContent('7')
    expect(chip(cy, 'Library 46 cards')).toHaveTextContent('46')
    expect(within(cy).getByRole('button', { name: 'Graveyard 5 cards, opens the list' })).toHaveTextContent(
      '5',
    )
    expect(within(cy).getByRole('button', { name: 'Exile 1 card, opens the list' })).toHaveTextContent('1')
    expect(within(cy).getByRole('group', { name: "Cy's zones" })).toBeInTheDocument()
  })

  it('keeps life the largest figure of the pod', () => {
    show(2, undefined, variant)
    const life = must(panel('Cy').querySelector<HTMLElement>('[data-slot="life"]'))
    expect(life).toHaveClass(variant === 'panel' ? 'text-life' : 'text-2xl')
  })

  it('draws no mana pool while nothing floats, and lists each kind held, in words, when it does', () => {
    const { unmount } = show(2, undefined, variant)
    expect(within(panel('Cy')).queryByRole('group', { name: 'Mana pool' })).toBeNull()
    expect(panel('Cy')).not.toHaveTextContent(/mana|empty/i)
    unmount()
    show(
      0,
      (view) => {
        must(view.players[0]).mana = { counts: [1, 0, 0, 3, 0, 2] }
      },
      variant,
    )
    const pool = within(panel('Alice (you)')).getByRole('group', { name: 'Mana pool' })
    expect(pool).toHaveTextContent('1 white')
    expect(pool).toHaveTextContent('3 red')
    expect(pool).toHaveTextContent('2 colorless')
    expect(pool).not.toHaveTextContent('blue')
  })

  it('carries data-seat for the decision layer, marks the viewer, and says nothing for a seat that is not at the table', () => {
    const { unmount } = show(3, undefined, variant)
    expect(panel('Dee')).toHaveAttribute('data-seat', '3')
    expect(panel('Dee')).toHaveAttribute('data-variant', variant)
    unmount()
    const { container, unmount: again } = show(9, undefined, variant)
    expect(container.querySelector('[data-seat]')).toBeNull()
    again()
    show(0, undefined, variant)
    expect(panel('Alice (you)')).toHaveAttribute('data-you', 'true')
    expect(panel('Alice (you)')).toHaveTextContent('you')
  })

  it('falls back to "Seat n" while the server has not named the seat', () => {
    renderTable({
      seats: 4,
      ui: <SeatPanel seat={2} variant={variant} />,
      lookup: { game: null, missing: false, failed: false },
    })
    expect(within(panel('Seat 3')).getByRole('heading', { name: 'Seat 3' })).toBeInTheDocument()
  })

  it.each([
    // Seat 0 holds priority without the turn: its dot says so, with a name of its own, so the words add nothing.
    [0, 'priority', ''],
    [1, 'active', 'Active turn.'],
    [2, undefined, ''],
  ])('marks seat %i with data-state %j, in words for a screen reader', (seat, state, words) => {
    show(seat, undefined, variant)
    const root = document.querySelector<HTMLElement>(`section[data-seat="${seat}"]`)
    expect(root?.getAttribute('data-state') ?? undefined).toBe(state)
    expect(root?.querySelector('p.sr-only')?.textContent ?? '').toBe(words)
  })

  it('lists both marks as tokens when the seat is active and holds priority', () => {
    show(
      1,
      (view) => {
        must(view.priority).holder = 1
      },
      variant,
    )
    const root = panel('Bo')
    expect(root).toHaveAttribute('data-state', 'active priority')
    expect(root).toHaveTextContent('Active turn. Has priority.')
    expect(root.matches('[data-state~="active"][data-state~="priority"]')).toBe(true)
  })

  it('draws no hand, neither the viewer’s cards nor card backs: the dock and the chip say it', () => {
    show(0, undefined, variant)
    expect(within(panel('Alice (you)')).queryByRole('list', { name: "Alice's hand" })).toBeNull()
    expect(panel('Alice (you)').querySelector('[data-slot="hand-backs"]')).toBeNull()
    // The commander of a panel is the only card it draws.
    expect(panel('Alice (you)').querySelectorAll('button[data-object-id]').length).toBeLessThanOrEqual(1)
  })

  it('shows red life of 5 or less', () => {
    show(
      3,
      (view) => {
        must(view.players[3]).life = 4
      },
      variant,
    )
    expect(panel('Dee').querySelector('[data-slot="life"] .text-destructive')).toHaveTextContent('4')
  })

  it.each<[LossReason, string]>([
    ['ZeroLife', 'Eliminated: ran out of life'],
    ['EmptyLibrary', 'Eliminated: could not draw from an empty library'],
    ['Conceded', 'Eliminated: conceded'],
    [{ Format: 'commander damage' }, 'Eliminated: lost: commander damage'],
  ])('says why a player is out: %j', (reason, text) => {
    show(
      2,
      (view) => {
        must(view.players[2]).lost = reason
      },
      variant,
    )
    expect(panel('Cy')).toHaveTextContent(text)
    expect(panel('Cy')).toHaveAttribute('data-eliminated', 'true')
  })

  it('shows no elimination for a player still in', () => {
    show(2, undefined, variant)
    expect(panel('Cy')).not.toHaveAttribute('data-eliminated')
    expect(panel('Cy')).not.toHaveTextContent('Eliminated')
  })

  it.each<[MulliganStatus, number, string]>([
    ['NeedsHand', 0, 'Mulligan: Waiting for a new hand'],
    ['Deciding', 1, 'Mulligan: Deciding whether to keep · 1 mulligan'],
    ['Mulliganing', 0, 'Mulligan: Mulligan taken, waiting for the table'],
    [{ Bottoming: 2 }, 2, 'Mulligan: Putting 2 cards on the bottom · 2 mulligans'],
    ['Kept', 0, 'Mulligan: Kept their hand'],
  ])('shows progress %j during the mulligan stage', (mulligan, mulligans, text) => {
    show(
      1,
      (view) => {
        view.stage = 'Mulligan'
        must(view.players[1]).mulligan = mulligan
        must(view.players[1]).mulligans = mulligans
      },
      variant,
    )
    expect(panel('Bo')).toHaveTextContent(text)
  })

  it('shows no mulligan line once turns are being played', () => {
    show(
      1,
      (view) => {
        must(view.players[1]).mulligan = 'Deciding'
      },
      variant,
    )
    expect(panel('Bo').querySelector('[data-slot="mulligan"]')).toBeNull()
  })

  it('has the pod of the viewer light up on their own turn, like the banner', () => {
    renderTable({
      fixture: buildFixture({ seats: 4, active: 0, permanents: 2 }),
      ui: <SeatPanel seat={0} variant={variant} />,
    })
    expect(panel('Alice (you)')).toHaveClass('your-turn-cue')
  })
})

describe('the command zone', () => {
  const noCommander = (view: StateView): void => {
    view.format = 'Normal'
    for (const player of view.players) player.command = { count: 0, cards: [] }
  }

  it('shows the commander of the panel as a card beside the chips', () => {
    show(1)
    const zone = within(panel('Bo')).getByRole('list', { name: "Bo's command zone" })
    expect(within(zone).getAllByRole('button')).toHaveLength(1)
    expect(zone).toHaveTextContent('Atraxa')
    expect(zone.style.getPropertyValue('--card-w')).toBe('44px')
  })

  it('shows it in the one-line pod as a chip that opens the list', async () => {
    const { user } = show(1, undefined, 'line')
    const bo = panel('Bo')
    expect(within(bo).queryByRole('list', { name: "Bo's command zone" })).toBeNull()
    await user.click(within(bo).getByRole('button', { name: 'Command zone 1 card, opens the list' }))
    const dialog = await screen.findByRole('dialog', { name: /command zone/i })
    expect(dialog).toHaveTextContent('Atraxa')
  })

  it.each<Variant>(VARIANTS)('draws nothing for an empty command zone in the %s pod', (variant) => {
    show(1, noCommander, variant)
    const bo = panel('Bo')
    expect(within(bo).queryByRole('list', { name: "Bo's command zone" })).toBeNull()
    expect(within(bo).queryByRole('button', { name: /^Command zone/ })).toBeNull()
    expect(bo.querySelector('[data-zone="command"]')).toBeNull()
  })

  it.each<[string, (view: StateView) => void]>([
    ['a Commander game', () => undefined],
    ['a game with no commanders', noCommander],
    ['a command zone card in a game not named Commander', (view) => (view.format = 'Normal')],
  ])('never writes a placeholder for commander damage or tax in %s', (_name, edit) => {
    for (const variant of VARIANTS) {
      const { unmount, container } = show(1, edit, variant)
      expect(container.querySelector('[data-slot="commander-stats"]')).toBeNull()
      expect(container.textContent).not.toMatch(/not tracked|commander damage and tax/i)
      unmount()
    }
  })
})

describe('graveyard and exile', () => {
  it('lists every card of the graveyard, most recent first, in a dialog', async () => {
    const { user, store } = show(1)
    const ids = must(must(store.getState().view).players[1]).graveyard.cards ?? []
    await user.click(within(panel('Bo')).getByRole('button', { name: /^Graveyard 5 cards/ }))
    const dialog = await screen.findByRole('dialog', { name: /graveyard/i })
    expect(dialog).toHaveTextContent('5 cards, most recent first.')
    const shown = within(within(dialog).getByRole('list')).getAllByRole('button')
    expect(shown.map((item) => Number(item.getAttribute('data-object-id')))).toEqual(ids.toReversed())
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it.each<Variant>(VARIANTS)('keeps an empty graveyard as a disabled chip in the %s pod', (variant) => {
    show(
      1,
      (view) => {
        must(view.players[1]).graveyard = { count: 0, cards: [] }
      },
      variant,
    )
    const button = within(panel('Bo')).getByRole('button', { name: 'Graveyard 0 cards' })
    expect(button).toBeDisabled()
    expect(button).toHaveTextContent('0')
  })

  it('counts only the exiled cards the seat owns', () => {
    renderTable({
      seats: 4,
      ui: (
        <>
          <SeatPanel seat={1} />
          <SeatPanel seat={3} />
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
    expect(within(panel('Bo')).getByRole('button', { name: 'Exile 3 cards, opens the list' })).toBeEnabled()
    expect(within(panel('Dee')).getByRole('button', { name: 'Exile 1 card, opens the list' })).toBeEnabled()
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
    await user.click(within(panel('Bo')).getByRole('button', { name: /^Exile 2 cards/ }))
    const dialog = await screen.findByRole('dialog', { name: /exile/i })
    expect(dialog).toHaveTextContent('Exiled A')
    expect(dialog).not.toHaveTextContent('Exiled C')
  })
})

describe('the one-line pod', () => {
  it('keeps the whole pod in a single row: identity first, then the chips', () => {
    // Without the commander damage it took, Cy has nothing for the status row.
    show(2, (view) => void (must(view.players[2]).commander_damage = undefined), 'line')
    const cy = panel('Cy')
    const row = must(cy.firstElementChild)
    expect(row).toContainElement(cy.querySelector('h3'))
    expect(row).toContainElement(within(cy).getByRole('group', { name: "Cy's zones" }))
    expect(cy.querySelector('[data-slot="pod-status"]')).toHaveClass('empty:hidden')
    expect(cy.querySelector('[data-slot="pod-status"]')).toBeEmptyDOMElement()
  })

  it('wraps the chips under the name only when its row is too narrow, never the name away', () => {
    show(2, undefined, 'line')
    const row = must(panel('Cy').firstElementChild)
    expect(row).toHaveClass('flex-wrap')
    expect(must(row.firstElementChild)).toHaveClass('min-w-24', 'flex-1')
    expect(within(panel('Cy')).getByRole('group', { name: "Cy's zones" })).toHaveClass('shrink-0')
  })

  it('puts the mana pool and the notes in a row of their own under it, only while there is one', () => {
    show(
      0,
      (view) => {
        must(view.players[0]).mana = { counts: [0, 0, 0, 1, 0, 0] }
      },
      'line',
    )
    const status = must(panel('Alice (you)').querySelector<HTMLElement>('[data-slot="pod-status"]'))
    expect(within(status).getByRole('group', { name: 'Mana pool' })).toBeInTheDocument()
  })
})

describe('the panel', () => {
  it('keeps room for the mana pool, so the chips do not move when mana floats', () => {
    show(2)
    const slot = must(panel('Cy').querySelector<HTMLElement>('[data-slot="mana-slot"]'))
    expect(slot).toHaveClass('min-h-5')
    expect(slot).toBeEmptyDOMElement()
    expect(slot.previousElementSibling).toBe(panel('Cy').querySelector('h3')?.parentElement)
  })
})
