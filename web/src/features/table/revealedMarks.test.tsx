// Goal: what Reveal and Note put in the view shows on the table: a seat's revealed hand cards and top card are a chip beside its hand and library in every pod (panel, one-line, tile) that lists them face up, a revealed card carries a "Revealed" mark and a note carries its first words, each named in words with the whole note, the icon alone on a small face, and the preview lists both.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice } from '@/features/cards/imageSetting'
import { indexById } from '@/play/view'
import type { ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { permanent } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from './Card'
import { buildFixture } from './dev/fixture'
import { OWN_NOTE, RIVAL_NOTE, withRevealed } from './dev/revealScene'
import type { Viewport } from './layout'
import type { CardSize } from './cardSize'

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

const scene = (seats: number) => withRevealed(buildFixture({ seats, permanents: 6 }))

const revealedOf = (seat: number, fixture: ReturnType<typeof scene>) =>
  must(fixture.view.players.find((player) => player.seat === seat))

describe('the chip beside the hand and library chips', () => {
  it.each<[string, number, Viewport, 'panel' | 'line']>([
    ['a duel pod', 2, { width: 1440, height: 900 }, 'panel'],
    ['a one-line pod', 4, { width: 1440, height: 900 }, 'line'],
  ])('is a button in %s that lists the cards face up', async (_name, seats, viewport, kind) => {
    const fixture = scene(seats)
    const { user } = renderTable({ fixture, options: { viewport } })
    const pod = must(
      document.querySelector<HTMLElement>(`section[data-seat="1"][data-variant="${kind}"]`),
      `pod of seat 1`,
    )
    const chip = within(pod).getByRole('button', {
      name: /^Revealed 3 cards: 2 in hand, the top of the library/,
    })
    expect(chip).toHaveTextContent('3')
    // The chip sits with the hand and library chips, not among the pictures of the corner.
    expect(within(pod).getByRole('img', { name: /^Hand / })).toBeInTheDocument()
    await user.click(chip)
    const list = screen.getByRole('dialog', { name: /revealed cards/ })
    const ids = revealedOf(1, fixture).revealed_hand ?? []
    const top = revealedOf(1, fixture).revealed_top
    for (const id of [...ids, ...(top === undefined ? [] : [top])]) {
      const card = must(list.querySelector<HTMLElement>(`button[data-object-id="${id}"]`), `card ${id}`)
      expect(within(card).getByRole('img', { name: 'Revealed to every seat' })).toBeInTheDocument()
    }
  })

  it('is a figure named in words in a tile, and the tile says so too', () => {
    renderTable({ fixture: scene(6), options: { viewport: { width: 1280, height: 720 } } })
    const tile = must(document.querySelector<HTMLElement>('[data-tile-seat="1"]'))
    expect(within(tile).getByRole('img', { name: 'Revealed 3 cards' })).toHaveTextContent('3')
    expect(tile).toHaveAccessibleName(/revealed 3 cards/)
    const other = must(document.querySelector<HTMLElement>('[data-tile-seat="2"]'))
    expect(within(other).queryByRole('img', { name: /^Revealed/ })).toBeNull()
  })

  it('is not drawn for a seat that shows nothing', () => {
    renderTable({ fixture: buildFixture({ seats: 2 }) })
    expect(document.querySelector('[data-zone="revealed"]')).toBeNull()
  })

  it('shows the viewer’s own pod with its one revealed card too', () => {
    renderTable({ fixture: scene(2) })
    const own = must(document.querySelector<HTMLElement>('section[data-seat="0"]'))
    expect(within(own).getByRole('button', { name: /^Revealed 1 card: 1 in hand/ })).toBeInTheDocument()
  })
})

describe('the mark on a revealed hand card', () => {
  it('is on the viewer’s revealed card in the hand dock and on no other', () => {
    const fixture = scene(2)
    renderTable({ fixture })
    const dock = must(document.querySelector<HTMLElement>('[data-region="hand"]'))
    const mine = revealedOf(0, fixture).revealed_hand ?? []
    expect(mine).toHaveLength(1)
    const marks = [...dock.querySelectorAll('[data-slot="card-revealed"]')]
    expect(marks).toHaveLength(1)
    expect(marks[0]?.closest('[data-object-id]')).toHaveAttribute('data-object-id', String(mine[0]))
    const card = must(dock.querySelector<HTMLElement>(`button[data-object-id="${mine[0]}"]`))
    expect(card).toHaveAccessibleName(/revealed to every seat/)
  })
})

const ID = 900
const IDENTITY = { oracle_id: 'giant-oracle' }

function renderCard(object: ObjectView, size: CardSize) {
  return renderTable({
    ui: <Card id={ID} size={size} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, { ...object, id: ID }]
      view.players = view.players.map((player) =>
        player.seat === 1 ? { ...player, revealed_top: ID } : player,
      )
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

/** Draws the card and lets its picture load, so the marks sit over the picture. */
async function loaded(object: ObjectView, size: CardSize) {
  const view = renderCard(object, size)
  act(() => fireEvent.load(must(view.container.querySelector('img'))))
  await act(async () => {
    await Promise.resolve()
  })
  return view
}

describe('the marks over a picture', () => {
  const card = permanent({}, { identity: IDENTITY, note: RIVAL_NOTE })

  it.each<[CardSize, boolean, boolean]>([
    // size, "Revealed" shows its word, the note shows its words
    ['compact', false, false],
    ['normal', false, true],
    ['large', true, true],
  ])(
    'at %s: the word of "Revealed" is %s and the first words of the note are %s',
    async (size, word, words) => {
      const { container } = await loaded(card, size)
      const foot = must(container.querySelector('[data-slot="card-marks-foot"]'))
      const revealed = must(foot.querySelector('[data-slot="card-revealed"]'))
      expect(revealed).toHaveAccessibleName('Revealed to every seat')
      expect(revealed.textContent === 'Revealed').toBe(word)
      const note = must(foot.querySelector('[data-slot="card-hand-note"]'))
      // The name holds the whole note whatever the face shows of it.
      expect(note).toHaveAccessibleName(`Note: ${RIVAL_NOTE}`)
      expect((note.textContent ?? '') !== '').toBe(words)
      if (words) expect(note).toHaveTextContent('Holds the trick…')
    },
  )

  it('draws neither mark for a card nobody revealed and no seat noted', async () => {
    const view = renderTable({
      ui: <Card id={ID} size="large" />,
      state: (state) => {
        const next = clone(must(state.view))
        next.objects = [...next.objects, { ...permanent({}, { identity: IDENTITY }), id: ID }]
        return { ...state, view: next, objectsById: indexById(next.objects) }
      },
    })
    act(() => fireEvent.load(must(view.container.querySelector('img'))))
    await act(async () => {
      await Promise.resolve()
    })
    expect(view.container.querySelector('[data-slot="card-revealed"]')).toBeNull()
    expect(view.container.querySelector('[data-slot="card-hand-note"]')).toBeNull()
  })

  it('puts the whole note in the card’s spoken name', async () => {
    const { container } = await loaded(card, 'normal')
    expect(must(container.querySelector('button[data-object-id]'))).toHaveAccessibleName(
      new RegExp(`note: ${RIVAL_NOTE}`),
    )
  })
})

describe('the mark on a text face', () => {
  it('is a chip in the art slot with the same names', () => {
    const fixture = scene(2)
    const noted = must(fixture.view.objects.find((object) => object.note === OWN_NOTE))
    const { container } = renderTable({ fixture, ui: <Card id={noted.id} size="normal" /> })
    const chip = must(container.querySelector('[data-slot="card-hand-note"]'))
    expect(chip).toHaveAccessibleName(`Note: ${OWN_NOTE}`)
    expect(chip).toHaveTextContent('Back to hand…')
  })
})

describe('the preview', () => {
  it('lists a revealed card and its owner’s hand, and a noted card with the whole note', async () => {
    const fixture = scene(2)
    const { user, container } = renderTable({ fixture })
    const own = revealedOf(0, fixture).revealed_hand?.[0]
    await user.hover(must(container.querySelector<HTMLElement>(`button[data-object-id="${own}"]`)))
    const preview = screen.getByTestId('card-preview')
    expect(preview.querySelector('[data-slot="preview-revealed"]')).toHaveTextContent(
      /^Revealed to every seat, in \w+'s hand\.$/,
    )
    await user.unhover(must(container.querySelector<HTMLElement>(`button[data-object-id="${own}"]`)))
    const rival = must(fixture.view.objects.find((object) => object.note === RIVAL_NOTE))
    await user.hover(must(container.querySelector<HTMLElement>(`button[data-object-id="${rival.id}"]`)))
    const noted = screen.getByTestId('card-preview')
    expect(noted.querySelector('[data-slot="preview-note"]')).toHaveTextContent(`Note: ${RIVAL_NOTE}`)
    expect(noted.querySelector('[data-slot="preview-revealed"]')).toBeNull()
  })
})
