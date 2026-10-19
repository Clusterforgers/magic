// Goal: in each form a seat takes (the 168 px pod of a duel, the one-line pod of three or four seats, the tile of the focus strip) the active seat has a 3 px edge in the colour of its turn (your pod the gold glow with the edge inside, an opponent's blue), a seat that holds priority without the turn has a dot named "Bo has priority", the active seat that holds priority too shows the edge alone, nothing shows before or after the turns, the marks are the same under reduced motion and none of them moves, and they read the turn and priority alone, which every seat sees.
import { within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { Stage, StateView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { ACTIVE_TINT } from './PodParts'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'

type Form = 'panel' | 'line' | 'tile'
const FORMS: Form[] = ['panel', 'line', 'tile']
/** The forms of a pod that can be the viewer's own; a tile stands for an opponent only. */
const PODS: Form[] = ['panel', 'line']

/** What a scene changes in the fixture: the fixture's own turn is Bo's (seat 1) and Alice (seat 0) holds priority. */
interface Scene {
  /** The seat whose turn it is. */
  active?: number
  /** The seat that holds priority; none for null. */
  holder?: number | null
  stage?: Stage
  /** Anything else to change in the view. */
  edit?: (view: StateView) => void
}

function show(form: Form, seat: number, scene: Scene = {}) {
  const fixture = buildFixture({
    seats: 5,
    permanents: 2,
    ...(scene.active === undefined ? {} : { active: scene.active }),
  })
  const state = (first: PlayState): PlayState => {
    const view = clone(must(first.view))
    if (scene.holder !== undefined)
      view.priority = scene.holder === null ? null : { holder: scene.holder, passed: [] }
    if (scene.stage !== undefined) view.stage = scene.stage
    scene.edit?.(view)
    return { ...first, view, objectsById: indexById(view.objects) }
  }
  const ui =
    form === 'tile' ? (
      <SeatTile seat={seat} focused={false} onFocusSeat={() => undefined} />
    ) : (
      <SeatPanel seat={seat} variant={form} />
    )
  return renderTable({ fixture, ui, state })
}

const root = (form: Form, seat: number): HTMLElement =>
  must(
    document.querySelector<HTMLElement>(
      form === 'tile' ? `[data-tile-seat="${seat}"]` : `section[data-seat="${seat}"]`,
    ),
  )
const dots = (form: Form, seat: number): HTMLElement[] => [
  ...root(form, seat).querySelectorAll<HTMLElement>('[data-slot="priority-dot"]'),
]
/** What the edge of a seat is, read from its classes. */
const edgeOf = (element: HTMLElement): 'yours' | 'theirs' | null =>
  element.classList.contains('your-turn-cue')
    ? 'yours'
    : element.classList.contains('inset-ring-3') && element.classList.contains('inset-ring-turn-them')
      ? 'theirs'
      : null

function reduceMotion(): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    media: query,
    matches: query.includes('reduce'),
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

afterEach(() => vi.unstubAllGlobals())

describe.each(FORMS)('the %s of the active opponent', (form) => {
  it('has a 3 px edge in the blue of an opponent’s turn, and says so in its marks', () => {
    show(form, 1)
    const seat = root(form, 1)
    expect(edgeOf(seat)).toBe('theirs')
    expect(seat).toHaveClass('inset-ring-3', 'inset-ring-turn-them')
    expect(seat).not.toHaveClass('your-turn-cue', 'inset-ring-turn-you-edge')
    expect(seat).toHaveAttribute('data-state', 'active')
  })

  it('draws no dot, since the edge already says whose turn it is', () => {
    show(form, 1)
    expect(dots(form, 1)).toHaveLength(0)
  })

  it('has neither the edge nor a dot on a seat that has no turn and no priority', () => {
    show(form, 2)
    expect(edgeOf(root(form, 2))).toBeNull()
    expect(root(form, 2).className).not.toMatch(/inset-ring/)
    expect(dots(form, 2)).toHaveLength(0)
    expect(root(form, 2)).not.toHaveAttribute('data-state')
  })

  it('shows the edge alone on the active seat that also holds priority', () => {
    show(form, 1, { holder: 1 })
    expect(edgeOf(root(form, 1))).toBe('theirs')
    expect(dots(form, 1)).toHaveLength(0)
    expect(root(form, 1)).toHaveAttribute('data-state', 'active priority')
    // The words of a seat in both states are still there for a screen reader.
    if (form === 'tile') expect(root(form, 1)).toHaveAccessibleName(expect.stringContaining('has priority'))
    else expect(root(form, 1)).toHaveTextContent('Active turn. Has priority.')
  })
})

describe.each(FORMS)('the %s of a seat that holds priority without the turn', (form) => {
  it('has a dot named after the seat and no edge', () => {
    show(form, 2, { holder: 2 })
    const [dot, ...more] = dots(form, 2)
    expect(more).toHaveLength(0)
    expect(within(root(form, 2)).getByRole('img', { name: 'Cy has priority' })).toBe(dot)
    expect(dot).toHaveAttribute('title', 'Cy has priority')
    expect(edgeOf(root(form, 2))).toBeNull()
    expect(root(form, 2)).toHaveAttribute('data-state', 'priority')
  })

  it('draws the dot as a shape of its own, so the cue is never colour alone', () => {
    show(form, 2, { holder: 2 })
    const [dot] = dots(form, 2)
    expect(dot).toHaveClass('rounded-full', 'size-2.5', 'shrink-0')
    // A named image, not a coloured seat dot (which is decoration beside the name).
    expect(dot).toHaveAttribute('role', 'img')
    expect(dot).not.toHaveAttribute('aria-hidden')
    expect(root(form, 2).querySelectorAll('[data-slot="seat-dot"]')).toHaveLength(1)
  })

  it('takes the dot away with the priority, and moves it to the next holder', () => {
    const table = show(form, 2, { holder: 2 })
    expect(dots(form, 2)).toHaveLength(1)
    table.update((state) => {
      const view = clone(must(state.view))
      view.priority = { holder: 3, passed: [2] }
      return { view }
    })
    expect(dots(form, 2)).toHaveLength(0)
    expect(root(form, 2)).not.toHaveAttribute('data-state')
  })
})

describe.each(PODS)('the %s of the viewer', (form) => {
  it('holds a dot named "You have priority" while an opponent has the turn', () => {
    show(form, 0)
    expect(edgeOf(root(form, 0))).toBeNull()
    expect(within(root(form, 0)).getByRole('img', { name: 'You have priority' })).toBeInTheDocument()
  })

  it('has the gold glow with a 3 px edge inside on its own turn, in place of any ring', () => {
    show(form, 0, { active: 0 })
    const seat = root(form, 0)
    expect(edgeOf(seat)).toBe('yours')
    expect(seat).toHaveClass('your-turn-cue', '[--cue-edge:3px]')
    expect(seat.className).not.toMatch(/(^|\s)(inset-)?ring-/)
    expect(seat).toHaveAttribute('data-state', 'active priority')
    expect(dots(form, 0)).toHaveLength(0)
  })

  it('leaves an opponent who holds priority on the viewer’s turn with a dot and no edge', () => {
    show(form, 2, { active: 0, holder: 2 })
    expect(edgeOf(root(form, 2))).toBeNull()
    expect(dots(form, 2)).toHaveLength(1)
  })
})

describe.each(FORMS)('the %s before and after the turns', (form) => {
  it.each<[string, Stage, number | undefined]>([
    ['the opening hands', 'Mulligan', undefined],
    ['a game that has not started', 'PreGame', undefined],
    ['a game that is over', 'Finished', 1],
  ])('shows no edge and no dot in %s', (_name, stage, winner) => {
    show(form, 1, {
      stage,
      holder: null,
      edit: (view) => {
        if (winner !== undefined) view.result = { Winner: winner }
      },
    })
    expect(edgeOf(root(form, 1))).toBeNull()
    expect(root(form, 1).className).not.toMatch(/inset-ring|your-turn-cue/)
    expect(root(form, 1)).not.toHaveAttribute('data-state')
    expect(dots(form, 1)).toHaveLength(0)
  })
})

describe.each(PODS)('the %s of the viewer when the result is in', (form) => {
  it('shows no gold edge, whatever the view still says of the turn', () => {
    show(form, 0, {
      active: 0,
      edit: (view) => {
        view.result = 'Draw'
      },
    })
    expect(edgeOf(root(form, 0))).toBeNull()
    expect(root(form, 0).className).not.toMatch(/your-turn-cue/)
  })
})

describe.each(FORMS)('the %s under reduced motion', (form) => {
  const marks = (seat: number, scene: Scene): string[] => {
    show(form, seat, scene)
    const element = root(form, seat)
    const names = [...element.querySelectorAll('[data-slot="priority-dot"]')].map((dot) => dot.className)
    const result = [element.className, ...names]
    document.body.innerHTML = ''
    return result
  }

  it.each<[string, number, Scene]>([
    ['the active opponent', 1, {}],
    ['the seat that holds priority', 2, { holder: 2 }],
    ['an idle seat', 3, {}],
  ])('draws the same marks on %s as with motion', (_name, seat, scene) => {
    const withMotion = marks(seat, scene)
    reduceMotion()
    expect(marks(seat, scene)).toEqual(withMotion)
  })

  it('has no class that moves: the edge and the dot are static, and the glow of the viewer’s pod is held to its stylesheet', () => {
    show(form, 1)
    show(form, 2, { holder: 2 })
    const moving = /animate-|transition|duration-|ease-|pulse/
    for (const seat of [root(form, 1), root(form, 2)]) expect(seat.className).not.toMatch(moving)
    expect(dots(form, 2)[0]?.className).not.toMatch(moving)
  })
})

describe.each(FORMS)('the %s and what the marks read', (form) => {
  it('is marked the same whatever else the seat’s figures say, since only the turn and priority count', () => {
    const plain = show(form, 2, { holder: 2 })
    const first = [root(form, 2).className, root(form, 2).getAttribute('data-state'), dots(form, 2).length]
    plain.unmount()
    show(form, 2, {
      holder: 2,
      edit: (view) => {
        const player = must(view.players[2])
        player.life = 3
        player.hand = { count: 0, cards: [] }
        player.mana = { counts: [4, 4, 4, 4, 4, 4] }
        must(view.players[1]).life = 99
      },
    })
    const second = [root(form, 2).className, root(form, 2).getAttribute('data-state'), dots(form, 2).length]
    expect(second).toEqual(first)
  })

  it('is told apart by the turn and priority of the view, the same for every seat that looks', () => {
    for (const viewer of [0, 3]) {
      const fixture = buildFixture({ seats: 5, permanents: 2, viewer, active: 2 })
      renderTable({
        fixture,
        ui:
          form === 'tile' ? (
            <SeatTile seat={2} focused={false} onFocusSeat={() => undefined} />
          ) : (
            <SeatPanel seat={2} variant={form} />
          ),
      })
      expect(root(form, 2).getAttribute('data-state')).toBe('active')
      expect(edgeOf(root(form, 2))).toBe('theirs')
      document.body.innerHTML = ''
    }
  })
})

describe('the tint of an active tile', () => {
  it('is the opponent’s turn colour at a few percent, with the edge, and only on the active tile', () => {
    show('tile', 1)
    expect(ACTIVE_TINT).toMatch(/^bg-turn-them\/\d$/)
    expect(root('tile', 1)).toHaveClass(ACTIVE_TINT)
    document.body.innerHTML = ''
    show('tile', 2)
    expect(root('tile', 2)).not.toHaveClass(ACTIVE_TINT)
  })
})
