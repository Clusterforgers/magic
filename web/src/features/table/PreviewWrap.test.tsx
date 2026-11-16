// Goal: whatever the card says, the table's preview panel never scrolls sideways and its details stay beside or under the picture: the picture's column is no wider than the picture, the details are one column that may shrink with words that break anywhere, and the oracle text wraps, for text beside or under the picture, hovered or pinned, on a desktop and a phone.
import { act, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { CardsApi } from '@/net/cards'
import type { CardDetail, ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from './GameTable'
import { CardsApiContext } from './cardsContext'
import { buildFixture } from './dev/fixture'

const original = { width: window.innerWidth, height: window.innerHeight }

function resize(width: number, height: number): void {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: width })
  Object.defineProperty(window, 'innerHeight', { configurable: true, value: height })
}

beforeEach(() => resize(1440, 900))
afterEach(() => resize(original.width, original.height))

/** A word with no place to break, and lines far wider than any panel. */
const UNBROKEN = 'Supercalifragilisticexpialidocious'.repeat(8)
const LONG_LINE =
  'Whenever another creature enters the battlefield under your control, you may gain 1 life. '.repeat(4)
const ORACLE = `${LONG_LINE}\n${UNBROKEN}`

const api: CardsApi = {
  search: () => Promise.reject(new Error('unused')),
  get: () =>
    Promise.resolve({
      faces: [
        {
          name: 'Suture Priest',
          mana_cost: '{1}{W}',
          type_line: 'Creature',
          oracle_text: ORACLE,
          power: '1',
          toughness: '1',
          loyalty: null,
          defense: null,
          colors: ['White'],
        },
      ],
    } as unknown as CardDetail),
}

/** The card made to say as much as a card can with nothing to break a line at. */
function wordy(object: ObjectView): ObjectView {
  return {
    ...object,
    identity: { oracle_id: 'suture' },
    subtypes: [UNBROKEN],
    stickers: [UNBROKEN],
    activated: [{ cost: UNBROKEN, effect: `${UNBROKEN} ${LONG_LINE}` }],
  }
}

/** Shows the preview of the opponent's first permanent, hovered or pinned, on a table where it is wordy. */
async function shown(pin: boolean) {
  const fixture = buildFixture({ seats: 2 })
  const first = must(
    fixture.view.objects.find((object) => object.zone === 'Battlefield' && object.owner === 1),
  )
  const view = {
    ...fixture.view,
    objects: fixture.view.objects.map((object) => (object.id === first.id ? wordy(object) : object)),
  }
  const { container, user, unmount } = renderTable({
    fixture: { ...fixture, view },
    ui: (
      <CardsApiContext value={api}>
        <GameTable />
      </CardsApiContext>
    ),
  })
  const card = must(container.querySelector<HTMLElement>(`button[data-object-id="${first.id}"]`))
  card.getBoundingClientRect = () => new DOMRect(100, 300, 60, 84)
  if (pin) await user.click(card)
  else await user.hover(card)
  await act(async () => {
    await Promise.resolve()
  })
  const panel = screen.getByTestId('card-preview')
  return {
    panel,
    unmount,
    picture: must(panel.querySelector<HTMLElement>('[data-slot="preview-picture"]')),
    details: must(panel.querySelector<HTMLElement>('[data-slot="preview-details"]')),
  }
}

describe('the preview panel with lines that never break', () => {
  it.each([
    ['a desktop, the text under the picture', 1440, 900, false],
    ['a phone, the text under the picture', 390, 844, false],
    ['a short window, the text beside the picture', 568, 320, true],
  ] as const)('stays inside its width on %s', async (_, width, height, beside) => {
    resize(width, height)
    for (const pin of [false, true]) {
      const { panel, picture, details, unmount } = await shown(pin)
      // The panel hides what it cannot wrap and never offers a sideways scroll.
      expect(panel).toHaveClass('overflow-x-hidden')
      expect(panel.classList.contains('flex')).toBe(beside)
      expect(panel.hasAttribute('data-pinned')).toBe(pin)
      // The oracle text is on show, breaks anywhere and sits in a column that may shrink to nothing.
      const oracle = must(panel.querySelector<HTMLElement>('[data-slot="preview-oracle"]'))
      expect(oracle).toHaveTextContent(UNBROKEN)
      expect(oracle).toHaveClass('[overflow-wrap:anywhere]')
      expect(must(oracle.closest('div.grid'))).toHaveClass('grid-cols-[minmax(0,1fr)]')
      // The details are one such column too, and take what the picture leaves.
      expect(details).toHaveTextContent(UNBROKEN)
      expect(details).toHaveClass('grid-cols-[minmax(0,1fr)]', '[overflow-wrap:anywhere]')
      expect(details.classList.contains('min-w-0')).toBe(beside)
      expect(details.classList.contains('flex-1')).toBe(beside)
      if (beside) {
        // A column exactly the picture wide: the text under the picture wraps in it and cannot push the details away.
        expect(picture.style.width).toBe(`${panel.dataset.picture}px`)
        expect(picture).toHaveClass('shrink-0')
      } else {
        expect(picture).toHaveClass('max-w-full')
        expect(picture.style.width).toBe('')
      }
      unmount()
    }
  })
})
