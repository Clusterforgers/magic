// Goal: the table lights the "your turn" cue (a glow class and a data flag) on the shared strip and on the viewer's own seat panel, and a "Your turn" badge in the top bar, exactly while the viewer is the active seat of a running game, never on an opponent's panel, and drops it the moment the turn passes, whatever the table size or seat.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'

const CUE = 'your-turn-cue'
/** The strip between the boards, the part of the shared region that carries the glow. */
const banner = () =>
  must(
    screen
      .getByRole('region', { name: 'Turn and priority' })
      .querySelector<HTMLElement>('[data-slot="shared-sheet"]'),
  )
/** The turn badge of the top bar. */
const badge = () => must(document.querySelector<HTMLElement>('[data-slot="banner"]'))
const panels = (container: HTMLElement): HTMLElement[] => [
  ...container.querySelectorAll<HTMLElement>('section[data-seat]'),
]
const lit = (container: HTMLElement): number[] =>
  panels(container)
    .filter((panel) => panel.hasAttribute('data-your-turn'))
    .map((panel) => Number(panel.getAttribute('data-seat')))

describe('on the viewer’s own turn', () => {
  it.each([
    [2, 0],
    [2, 1],
    [3, 2],
    [4, 0],
    [6, 3],
    [12, 11],
  ])('at %i seats, seat %i sees the banner and only its own panel lit', (seats, viewer) => {
    const fixture = buildFixture({ seats, viewer, active: viewer, permanents: 2 })
    const { container } = renderTable({ fixture })
    expect(banner()).toHaveAttribute('data-your-turn', 'true')
    expect(banner()).toHaveClass(CUE)
    expect(badge()).toHaveTextContent(/^Your turn · /)
    expect(lit(container)).toEqual([viewer])
    expect(must(container.querySelector(`section[data-seat="${viewer}"]`))).toHaveClass(CUE)
  })

  it('leaves the ring classes off the lit elements, since the glow draws its own ring', () => {
    const { container } = renderTable({ fixture: buildFixture({ seats: 2, active: 0, permanents: 2 }) })
    for (const element of [banner(), must(container.querySelector<HTMLElement>('section[data-seat="0"]'))]) {
      expect(element).toHaveClass(CUE)
      expect(element.className).not.toMatch(/(^|\s)ring-/)
    }
  })

  it('keeps the viewer’s own panel marked as the active turn as before', () => {
    renderTable({ fixture: buildFixture({ seats: 2, active: 0, permanents: 2 }) })
    expect(screen.getByRole('region', { name: 'Alice (you)' })).toHaveTextContent('Active turn')
  })
})

describe('on someone else’s turn', () => {
  it.each([2, 3, 6])('lights nothing at %i seats, and the active opponent has the blue edge', (seats) => {
    const { container } = renderTable({ fixture: buildFixture({ seats, permanents: 2 }) })
    expect(container.querySelector('[data-your-turn]')).toBeNull()
    expect(container.querySelector(`.${CUE}`)).toBeNull()
    expect(badge()).not.toHaveTextContent(/^Your turn/)
    // Another seat's turn leaves the strip with the edge of the phase alone, and no gold glow.
    expect(banner()).toHaveClass('border-phase')
    const active = must(container.querySelector<HTMLElement>('section[data-seat][data-active]'))
    expect(active).toHaveAttribute('data-seat', '1')
    expect(active).toHaveClass('inset-ring-3', 'inset-ring-turn-them')
    expect(active).not.toHaveClass('inset-ring-turn-you-edge')
  })
})

describe('when the turn changes', () => {
  const passTo = (active: number) => (state: PlayState) => {
    const view = clone(must(state.view))
    view.turn = { ...view.turn, active }
    return { view }
  }

  it('lights up when the turn comes round and goes out when it leaves', () => {
    const { container, update } = renderTable({ seats: 3 })
    expect(lit(container)).toEqual([])
    update(passTo(0))
    expect(lit(container)).toEqual([0])
    expect(banner()).toHaveAttribute('data-your-turn', 'true')
    expect(badge()).toHaveTextContent(/^Your turn · /)
    update(passTo(2))
    expect(lit(container)).toEqual([])
    expect(banner()).not.toHaveAttribute('data-your-turn')
    expect(badge()).not.toHaveTextContent(/^Your turn/)
    expect(banner()).toHaveClass('border-phase')
    expect(banner()).not.toHaveClass(CUE)
    update(passTo(0))
    expect(lit(container)).toEqual([0])
  })

  it('stays lit through the steps of the viewer’s own turn', () => {
    const { container, update } = renderTable({
      fixture: buildFixture({ seats: 2, active: 0, permanents: 2 }),
    })
    for (const step of ['Upkeep', 'Main1', 'DeclareAttackers', 'End'] as const) {
      update((state) => {
        const view = clone(must(state.view))
        view.turn = { ...view.turn, step }
        return { view }
      })
      expect(lit(container), step).toEqual([0])
    }
  })
})

describe('outside a running turn', () => {
  it.each<[string, (state: PlayState) => Partial<PlayState>]>([
    ['the opening hands', (state) => ({ view: { ...must(state.view), stage: 'Mulligan' } })],
    ['before the game starts', (state) => ({ view: { ...must(state.view), stage: 'PreGame' } })],
    [
      'a finished game',
      (state) => ({ view: { ...must(state.view), stage: 'Finished', result: { Winner: 1 } } }),
    ],
    [
      'a result in a view that still says Turns',
      (state) => ({ view: { ...must(state.view), result: 'Draw' } }),
    ],
  ])('shows no cue in %s even if the viewer is the active seat', (_when, change) => {
    const { container, update } = renderTable({
      fixture: buildFixture({ seats: 2, active: 0, permanents: 2 }),
    })
    expect(lit(container)).toEqual([0])
    update(change)
    expect(lit(container)).toEqual([])
    expect(container.querySelector('[data-your-turn]')).toBeNull()
  })
})
