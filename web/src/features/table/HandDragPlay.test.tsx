// Goal: dragging a hand card up out of the dock onto the table sends the same answer as a click on it (a land played, a spell cast, a commander cast from the head of the dock) in every layout, the viewer's board is marked as the drop target and the card says "Release to play"; a card that cannot be played says so and sends nothing, a drag along the hand or one let go back over the hand sends nothing, a pick among the hand is no play, and nothing is sent before the drop.
import { act, fireEvent } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, ObjectId, ObjectView } from '@/protocol/generated'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { TableHarness } from '@/test-utils/tableHarness'
import { withBoss } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import type { Fixture } from './dev/fixture'
import type { Viewport } from './layout'

const isLand = (object: ObjectView): boolean => object.types.includes('Land')

/** The viewer's hand, and its commanders waiting in the command zone. */
function cardsOf(state: PlayState): { hand: ObjectView[]; command: ObjectId[] } {
  const view = must(state.view)
  const player = must(view.players[view.viewer])
  const hand = (player.hand.cards ?? []).map((id) => must(state.objectsById[id]))
  const commanders = (player.commanders ?? []).map((entry) => entry.object)
  return { hand, command: (player.command.cards ?? []).filter((id) => commanders.includes(id)) }
}

/** Priority with every land of the hand playable and every spell but the last castable, the commanders too. */
function atPriority(state: PlayState): PlayState {
  const { hand, command } = cardsOf(state)
  const spells = hand.filter((object) => !isLand(object))
  return {
    ...state,
    pending: {
      id: 1,
      decision: {
        player: must(state.view).viewer,
        kind: {
          Priority: {
            playable_lands: hand.filter(isLand).map((object) => object.id),
            castable: [...command, ...spells.slice(0, -1).map((object) => object.id)],
            mana_abilities: [],
            can_act: true,
            can_pass: true,
          },
        },
      },
    },
  }
}

const button = (id: ObjectId): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-region="hand"] button[data-object-id="${id}"]`))

/** Picks the card up and carries it by `dx`, `dy` (jsdom's dock has its top at 0, so the press starts just under it). */
function carry(id: ObjectId, dx: number, dy: number): void {
  const slot = must(button(id).closest<HTMLElement>('[data-slot="hand-slot"]'))
  fireEvent.mouseDown(slot, { button: 0, clientX: 100, clientY: 20 })
  fireEvent.mouseMove(document, { clientX: 108, clientY: 20 })
  fireEvent.mouseMove(document, { clientX: 100 + dx, clientY: 20 + dy })
}

/** Lets the card go, and waits out the moment dnd-kit swallows the click that ends a drag, so the next press counts. */
const letGo = (): Promise<void> =>
  act(async () => {
    fireEvent.mouseUp(document)
    await new Promise((resolve) => setTimeout(resolve, 80))
  })

const face = (): HTMLElement | null => document.querySelector<HTMLElement>('[data-slot="hand-drag-face"]')
const word = (): string | undefined =>
  document.querySelector('[data-slot="hand-drag-word"]')?.textContent ?? undefined

type Answers = ReturnType<typeof vi.fn<(given: Answer) => boolean>>

function table(fixture: Fixture, viewport: Viewport): TableHarness & { answer: Answers } {
  const answer = vi.fn<(given: Answer) => boolean>(() => true)
  const h = renderTable({ fixture, options: { viewport }, session: { answer }, state: atPriority })
  return { ...h, answer }
}

/** What a click on the card answers, on a table of its own. */
function clicked(
  fixture: () => Fixture,
  viewport: Viewport,
  pick: (state: PlayState) => ObjectId,
): Answer[][] {
  const h = table(fixture(), viewport)
  act(() => button(pick(h.store.getState())).click())
  const calls = [...h.answer.mock.calls]
  h.unmount()
  return calls
}

const LAYOUTS: ReadonlyArray<[string, () => Fixture, Viewport]> = [
  ['a duel', () => buildFixture({ seats: 2 }), { width: 1400, height: 900 }],
  ['a duel on a small laptop', () => buildFixture({ seats: 2 }), { width: 1280, height: 720 }],
  ['a grid of four', () => buildFixture({ seats: 4 }), { width: 1920, height: 1080 }],
  ['six seats', () => buildFixture({ seats: 6 }), { width: 1366, height: 768 }],
  ['a boss fight', () => withBoss(buildFixture({ seats: 4 }), 0), { width: 1400, height: 900 }],
  ['a phone', () => buildFixture({ seats: 2 }), { width: 390, height: 844 }],
]

const land = (state: PlayState): ObjectId => must(cardsOf(state).hand.find(isLand)).id
const spell = (state: PlayState): ObjectId => must(cardsOf(state).hand.find((object) => !isLand(object))).id
const lastSpell = (state: PlayState): ObjectId =>
  must(
    cardsOf(state)
      .hand.filter((object) => !isLand(object))
      .at(-1),
  ).id

describe.each(LAYOUTS)('a hand card dragged onto the table at %s', (_name, fixture, viewport) => {
  it.each<[string, (state: PlayState) => ObjectId]>([
    ['a land', land],
    ['a spell', spell],
  ])('plays %s as a click does, and says nothing before the drop', async (_kind, pick) => {
    const want = clicked(fixture, viewport, pick)
    expect(want).toHaveLength(1)
    const h = table(fixture(), viewport)
    carry(pick(h.store.getState()), 0, -200)
    expect(face()).toHaveAttribute('data-play', 'can')
    expect(word()).toBe('Release to play')
    expect(h.answer).not.toHaveBeenCalled()
    await letGo()
    expect(h.answer.mock.calls).toEqual(want)
    expect(face()).toBeNull()
  })

  it('returns a card that cannot be played and sends nothing', async () => {
    const h = table(fixture(), viewport)
    carry(lastSpell(h.store.getState()), 0, -200)
    expect(face()).toHaveAttribute('data-play', 'cannot')
    expect(word()).not.toBe('Release to play')
    await letGo()
    expect(h.answer).not.toHaveBeenCalled()
    expect(face()).toBeNull()
  })

  it('plays nothing on a drag along the hand, nor when the card comes back over the hand', async () => {
    const h = table(fixture(), viewport)
    const id = land(h.store.getState())
    carry(id, 60, 0)
    expect(face()).not.toBeNull()
    expect(face()).not.toHaveAttribute('data-play')
    await letGo()
    carry(id, 0, -200)
    expect(face()).toHaveAttribute('data-play', 'can')
    fireEvent.mouseMove(document, { clientX: 100, clientY: 24 })
    expect(face()).not.toHaveAttribute('data-play')
    await letGo()
    expect(h.answer).not.toHaveBeenCalled()
  })
})

describe('a card at the head of the dock', () => {
  it('is cast by a drag onto the table as by a click', async () => {
    const commander = (state: PlayState): ObjectId => must(cardsOf(state).command[0])
    const fixture = () => buildFixture({ seats: 4 })
    const viewport = { width: 1400, height: 900 }
    const want = clicked(fixture, viewport, commander)
    expect(want).toHaveLength(1)
    const h = table(fixture(), viewport)
    carry(commander(h.store.getState()), 0, -200)
    expect(face()).toHaveAttribute('data-play', 'can')
    await letGo()
    expect(h.answer.mock.calls).toEqual(want)
  })
})

describe('a pick among the hand', () => {
  it('is no play: a card dragged onto the table during a discard returns unchosen', async () => {
    const answer = vi.fn<(given: Answer) => boolean>(() => true)
    const h = renderTable({
      state: (state) => ({
        ...state,
        pending: {
          id: 1,
          decision: {
            player: must(state.view).viewer,
            kind: { Discard: { hand: cardsOf(state).hand.map((object) => object.id), count: 1 } },
          },
        },
      }),
      session: { answer },
    })
    const id = must(cardsOf(h.store.getState()).hand[0]).id
    carry(id, 0, -200)
    expect(face()).toHaveAttribute('data-play', 'cannot')
    await letGo()
    expect(answer).not.toHaveBeenCalled()
    expect(button(id)).not.toHaveAttribute('aria-pressed', 'true')
  })
})
