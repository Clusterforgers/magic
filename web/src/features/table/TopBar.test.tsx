// Goal: the top bar of the table has home, a turn badge that says whose turn it is and which step, with the phase's icon and name in its colour (filled on your own turn, tinted on another's) over an accent rule in the bar, the button of the side sheet where the side area is a sheet, and a table menu with the layout, the theme and Concede only, at every window size, since the stop ladder and Full Control live in the controls dock, and the Control keys work with the menu shut.
import { act, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { appTheme } from '@/hooks/useTheme'
import type { FullControlMode } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import type { Viewport } from './layout'

const WIDE: Viewport = { width: 1920, height: 1080 }
const LAPTOP: Viewport = { width: 1280, height: 720 }
const PHONE: Viewport = { width: 390, height: 844 }

afterEach(() => {
  appTheme().setChoice('system')
})

const badge = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-slot="banner"]'))
const bar = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-region="bar"]'))
const menuButton = () => screen.getByRole('button', { name: 'Table menu' })
const menu = () => screen.getByRole('dialog', { name: 'Table menu' })

describe('the bar', () => {
  it('has home, the turn badge and the table menu, in that order', () => {
    renderTable({ seats: 2, options: { viewport: WIDE } })
    const home = within(bar()).getByRole('link', { name: 'Home' })
    expect(home).toHaveAttribute('href', '/')
    const order = [home, badge(), menuButton()]
    for (let index = 1; index < order.length; index += 1) {
      const before = must(order[index - 1])
      expect(
        before.compareDocumentPosition(must(order[index])) & Node.DOCUMENT_POSITION_FOLLOWING,
      ).toBeTruthy()
    }
  })

  it('says whose turn and which step on a pill: filled with the phase’s colour for yours, tinted for another seat', () => {
    renderTable({
      fixture: buildFixture({ seats: 2, active: 0, permanents: 2 }),
      options: { viewport: WIDE },
    })
    expect(badge()).toHaveTextContent('Your turn · 5 · Declare blockers')
    expect(within(badge()).getByText(/^Your turn/)).toHaveClass('bg-phase', 'text-on-phase', 'border-phase')
  })

  it("names the other seat on an opponent's turn", () => {
    renderTable({ seats: 3, options: { viewport: WIDE } })
    expect(badge()).toHaveTextContent("Bo's turn · 5 · Declare blockers")
    expect(within(badge()).getByText(/^Bo's turn/)).toHaveClass(
      'bg-phase-soft',
      'text-foreground',
      'border-phase',
    )
  })

  it('puts the phase’s icon and name in the same capsule, ahead of the words, and the accent rule under the bar', () => {
    renderTable({ seats: 2, options: { viewport: WIDE } })
    const tag = must(document.querySelector<HTMLElement>('[data-slot="phase-tag"]'))
    expect(tag).toHaveTextContent('Combat')
    expect(tag.querySelector('svg')).not.toBeNull()
    expect(tag.compareDocumentPosition(badge()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(bar()).toHaveClass('border-b-[3px]', 'border-phase')
  })

  it('follows the game: the next step, the next turn, the next seat', () => {
    const { update } = renderTable({ seats: 3, options: { viewport: WIDE } })
    update((state) => ({
      view: state.view && { ...state.view, turn: { ...state.view.turn, step: 'End', number: 6, active: 2 } },
    }))
    expect(badge()).toHaveTextContent("Cy's turn · 6 · End step")
    update((state) => ({
      view: state.view && {
        ...state.view,
        turn: { ...state.view.turn, step: 'Upkeep', number: 7, active: 0 },
      },
    }))
    expect(badge()).toHaveTextContent('Your turn · 7 · Upkeep')
  })

  it('is not a live region: the turn is on screen all the time and says nothing aloud', () => {
    renderTable({ seats: 2, options: { viewport: WIDE } })
    expect(badge().closest('[aria-live]')).toBeNull()
    expect(badge()).not.toHaveAttribute('role')
  })

  it.each([
    ['a wide window', WIDE, false],
    ['a laptop', LAPTOP, false],
    ['a tablet', { width: 800, height: 900 }, true],
    ['a phone', PHONE, true],
  ])('has the button of the side sheet on %s: %s', (_name, viewport, has) => {
    renderTable({ seats: 2, options: { viewport } })
    const sheet = within(bar()).queryByRole('button', { name: /^Commands, log, chat and stack/ })
    expect(sheet !== null).toBe(has)
  })

  it('has smaller buttons in the 32 px bar of a phone on its side, and the usual ones elsewhere', () => {
    const { unmount } = renderTable({ seats: 2, options: { viewport: { width: 844, height: 390 } } })
    expect(menuButton()).toHaveClass('size-8')
    expect(within(bar()).getByRole('button', { name: /^Commands, log, chat and stack/ })).toHaveClass(
      'size-8',
    )
    unmount()
    renderTable({ seats: 2, options: { viewport: { width: 800, height: 900 } } })
    expect(menuButton()).toHaveClass('size-9')
  })

  it('names home by its words on a phone, where the word "Magic" is left out', () => {
    renderTable({ seats: 2, options: { viewport: PHONE } })
    expect(within(bar()).getByRole('link', { name: 'Home' })).toBeInTheDocument()
  })
})

describe('the table menu', () => {
  it('opens a labelled panel with the layout, the theme and Concede, and closes on Escape', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport: WIDE } })
    expect(screen.queryByRole('dialog', { name: 'Table menu' })).toBeNull()
    await user.click(menuButton())
    const panel = within(menu())
    expect(panel.getByRole('group', { name: 'Layout' })).toBeInTheDocument()
    expect(panel.getByRole('group', { name: 'Theme' })).toBeInTheDocument()
    expect(panel.getByRole('button', { name: 'Concede' })).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: 'Table menu' })).toBeNull()
  })

  it('shows the mode on screen as pressed and sets the other when it is pressed', async () => {
    const { user, container } = renderTable({ seats: 2, options: { viewport: WIDE } })
    await user.click(menuButton())
    const panel = within(menu())
    expect(panel.getByRole('button', { name: 'Grid' })).toHaveAttribute('aria-pressed', 'true')
    expect(panel.getByRole('button', { name: 'Focus' })).toHaveAttribute('aria-pressed', 'false')
    await user.click(panel.getByRole('button', { name: 'Focus' }))
    expect(panel.getByRole('button', { name: 'Focus' })).toHaveAttribute('aria-pressed', 'true')
    expect(container.querySelector('[data-layout]')).toHaveAttribute('data-layout', 'focus')
  })

  it('sets the theme from three buttons, shows the choice as pressed and puts the dark class on the page', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport: WIDE } })
    await user.click(menuButton())
    const panel = within(menu())
    expect(panel.getByRole('button', { name: 'System' })).toHaveAttribute('aria-pressed', 'true')
    await user.click(panel.getByRole('button', { name: 'Dark' }))
    expect(panel.getByRole('button', { name: 'Dark' })).toHaveAttribute('aria-pressed', 'true')
    expect(document.documentElement).toHaveClass('dark')
    await user.click(panel.getByRole('button', { name: 'Light' }))
    expect(document.documentElement).not.toHaveClass('dark')
    expect(appTheme().getChoice()).toBe('light')
  })

  it('has no Concede for a seat that is out', async () => {
    const { user } = renderTable({
      seats: 2,
      options: { viewport: WIDE },
      state: (state) => ({
        ...state,
        view: state.view && {
          ...state.view,
          players: state.view.players.map((player) =>
            player.seat === 0 ? { ...player, lost: 'ZeroLife' as const } : player,
          ),
        },
      }),
    })
    await user.click(menuButton())
    expect(within(menu()).queryByRole('button', { name: 'Concede' })).toBeNull()
  })
})

describe('the stop ladder and Full Control', () => {
  it.each([
    ['a wide window', WIDE],
    ['a laptop', LAPTOP],
    ['a phone', PHONE],
  ])('are never in the menu, which keeps the layout, the theme and Concede: %s', async (_name, viewport) => {
    const { user } = renderTable({ seats: 2, options: { viewport } })
    await user.click(menuButton())
    expect(within(menu()).queryByRole('region', { name: 'Stops and Full Control' })).toBeNull()
    expect(within(menu()).queryByRole('group', { name: 'Stops' })).toBeNull()
    expect(within(menu()).queryByRole('button', { name: /Full Control|^Stops/ })).toBeNull()
    expect(menu().querySelector('[data-slot="stop-ladder"]')).toBeNull()
    expect(
      [...menu().querySelectorAll('[role="group"][aria-label]')].map((group) =>
        group.getAttribute('aria-label'),
      ),
    ).toEqual(['Layout', 'Theme', 'Card images'])
    expect(within(menu()).getByRole('button', { name: 'Concede' })).toBeInTheDocument()
    // They are in the dock instead, once.
    expect(document.querySelectorAll('[data-slot="stop-ladder"]')).toHaveLength(1)
  })

  it('answer Control from the keyboard with the menu shut, wherever they are', async () => {
    const asked: FullControlMode[] = []
    const setFullControl = vi.fn((mode: FullControlMode) => {
      asked.push(mode)
      return true
    })
    const { user } = renderTable({
      seats: 2,
      options: { viewport: LAPTOP },
      session: { setFullControl },
      state: (state) => ({ ...state, status: 'open', pending: null }),
    })
    act(() => (document.activeElement as HTMLElement | null)?.blur())
    await user.keyboard('{Control}')
    expect(asked).toEqual(['NextWindow'])
  })
})
