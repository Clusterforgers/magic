// Goal: the table's preview lists what changes a card under its rules text, each line with its effect, its source (identical sources counted "×2") and its duration, in every window layout (text under or beside the picture, desktop and phone) and table (a duel, four seats, a boss fight); a line points at its source on the board while hovered, focused or tapped, and stops when the preview closes; a card nothing changes has no list; and the panel keeps its place and size when the list arrives.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { ModifierView, ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { withBoss } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import type { Fixture } from './dev/fixture'
import { modifierLines, useSourceLit } from './modifiers'

const original = { width: window.innerWidth, height: window.innerHeight }

function resize(width: number, height: number): void {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: width })
  Object.defineProperty(window, 'innerHeight', { configurable: true, value: height })
}

beforeEach(() => resize(1440, 900))
afterEach(() => {
  resize(original.width, original.height)
  useSourceLit.getState().light([])
})

const LAYOUTS = [
  ['a desktop', 1440, 900],
  ['a phone', 390, 844],
  ['a short window, text beside the picture', 568, 320],
] as const

const TABLES: readonly (readonly [string, () => Fixture])[] = [
  ['a duel', () => buildFixture({ seats: 2 })],
  ['four seats', () => buildFixture({ seats: 4 })],
  ['a boss fight', () => withBoss(buildFixture({ seats: 3 }), 1)],
]

/** The viewer's first permanent and another permanent on the board, which is the source of its modifiers. */
function pick(fixture: Fixture): { card: ObjectView; source: ObjectView } {
  const on = fixture.view.objects.filter((object) => object.zone === 'Battlefield')
  const card = must(on.find((object) => object.controller === fixture.view.viewer))
  const source = must(on.find((object) => object.id !== card.id))
  return { card, source }
}

function modifiersFrom(source: ObjectView): ModifierView[] {
  return [
    { effect: '+2/+2', source: source.name, sources: [source.id], count: 2 },
    { effect: '+3/+3, Trample', source: 'Giant Growth', count: 1, duration: 'until end of turn' },
    { effect: "Can't block", source: 'A face-down card', count: 1 },
    { effect: '+3/+3', source: '+1/+1 counter', count: 3 },
  ]
}

/** Renders the table with `card` changed by `modifiers` and pins its preview. */
async function pinned(fixture: Fixture, card: ObjectView, modifiers: ModifierView[] | undefined) {
  const view = {
    ...fixture.view,
    objects: fixture.view.objects.map((object) =>
      object.id === card.id ? { ...object, ...(modifiers === undefined ? {} : { modifiers }) } : object,
    ),
  }
  const harness = renderTable({ fixture: { ...fixture, view } })
  const face = must(harness.container.querySelector<HTMLElement>(`button[data-object-id="${card.id}"]`))
  face.getBoundingClientRect = () => new DOMRect(100, 300, 60, 84)
  await harness.user.click(face)
  await act(async () => {
    await Promise.resolve()
  })
  return { ...harness, panel: screen.getByTestId('card-preview') }
}

describe('the modifier lines (pure)', () => {
  it('count identical sources, name the duration and keep the source ids', () => {
    const lines = modifierLines({
      modifiers: [
        { effect: '+2/+2', source: 'Glorious Anthem', sources: [7, 8], count: 2 },
        { effect: '+3/+3', source: 'Giant Growth', count: 1, duration: 'until end of turn' },
      ],
    } as ObjectView)
    expect(lines.map((line) => line.words)).toEqual([
      '+2/+2 from Glorious Anthem ×2',
      '+3/+3 from Giant Growth, until end of turn',
    ])
    expect(lines[0]?.sources).toEqual([7, 8])
    expect(lines[1]?.sources).toEqual([])
  })

  it('are none for a card nothing changes', () => {
    expect(modifierLines({} as ObjectView)).toEqual([])
  })
})

describe.each(LAYOUTS)('the preview on %s', (_, width, height) => {
  it.each(TABLES)('lists what changes the card at %s', async (_table, build) => {
    resize(width, height)
    const fixture = build()
    const { card, source } = pick(fixture)
    const { panel, container, user, unmount } = await pinned(fixture, card, modifiersFrom(source))
    const list = within(panel).getByRole('list', { name: 'Modified by' })
    const items = within(list).getAllByRole('listitem')
    // What the eye reads; a screen reader hears the whole line ("+2/+2 from …").
    const shown = items.map((item) => item.querySelector('[aria-hidden="true"]')?.textContent)
    expect(shown).toEqual([
      `+2/+2 · ${source.name} ×2`,
      '+3/+3, Trample · Giant Growth, until end of turn',
      "Can't block · A face-down card",
      '+3/+3 · +1/+1 counter ×3',
    ])
    // The list comes after the rules text of the card.
    const abilities = panel.querySelector('[aria-label="Abilities"]')
    if (abilities !== null) {
      expect(abilities.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    }
    // Only a line whose source is on the table points; hovering it marks that card, leaving it unmarks.
    const buttons = within(list).getAllByRole('button')
    expect(buttons).toHaveLength(1)
    const board = (): HTMLElement =>
      must(container.querySelector<HTMLElement>(`button[data-object-id="${source.id}"]`))
    const pointer = must(buttons[0])
    fireEvent.pointerEnter(pointer, { pointerType: 'mouse' })
    expect(board()).toHaveAttribute('data-source-lit', 'true')
    fireEvent.pointerLeave(pointer, { pointerType: 'mouse' })
    expect(board()).not.toHaveAttribute('data-source-lit')
    // A tap toggles, as a touch has no hover.
    fireEvent.pointerDown(pointer, { pointerType: 'touch' })
    fireEvent.click(pointer)
    expect(board()).toHaveAttribute('data-source-lit', 'true')
    expect(pointer).toHaveAttribute('aria-pressed', 'true')
    fireEvent.pointerDown(pointer, { pointerType: 'touch' })
    fireEvent.click(pointer)
    expect(board()).not.toHaveAttribute('data-source-lit')
    // Closing the preview stops the pointing.
    act(() => pointer.focus())
    expect(board()).toHaveAttribute('data-source-lit', 'true')
    await user.click(within(panel).getByRole('button', { name: /close preview/i }))
    expect(board()).not.toHaveAttribute('data-source-lit')
    unmount()
  })
})

describe('a card nothing changes', () => {
  it('has no list', async () => {
    const fixture = buildFixture({ seats: 2 })
    const { card } = pick(fixture)
    const { panel, unmount } = await pinned(fixture, card, undefined)
    expect(within(panel).queryByRole('list', { name: 'Modified by' })).toBeNull()
    unmount()
  })
})

describe.each(LAYOUTS)('the panel on %s', (_, width, height) => {
  it('keeps its place and size when the list arrives', async () => {
    resize(width, height)
    const fixture = buildFixture({ seats: 2 })
    const { card, source } = pick(fixture)
    const { panel, update, unmount } = await pinned(fixture, card, undefined)
    const before = { ...panel.dataset, style: panel.getAttribute('style') }
    update((state) => ({
      objectsById: { ...state.objectsById, [card.id]: { ...card, modifiers: modifiersFrom(source) } },
    }))
    const now = screen.getByTestId('card-preview')
    expect(within(now).getByRole('list', { name: 'Modified by' })).toBeInTheDocument()
    expect({ ...now.dataset, style: now.getAttribute('style') }).toEqual(before)
    unmount()
  })
})
