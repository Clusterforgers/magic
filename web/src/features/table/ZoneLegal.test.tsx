// Goal: a graveyard or exile chip and a zone's picture show, without the list being opened, that a card inside can be played or used now: a ring and a play mark, the count in the name and the tooltip, and a sentence under the list's title; they say nothing for a prompt that picks among choices, for the command zone, for a card of another mark or once the mark goes.
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { ZoneButton } from './ZoneButton'
import type { ListedZone } from './ZoneButton'
import { ZonePicture } from './ZonePicture'
import { createMarksStore, MarksContext, markCards } from './prompt/marks'
import type { CardMarks } from './prompt/marks'

const fixture = buildFixture({ seats: 2 })
const graveyard = must(fixture.view.players[0]?.graveyard.cards)
const commanders = must(fixture.view.players[0]?.command.cards)
const exile = fixture.view.objects
  .filter((object) => object.zone === 'Exile' && object.owner === 0)
  .map((object) => object.id)

const IDS: Record<ListedZone, readonly ObjectId[]> = { graveyard, exile, command: commanders }

type Flavour = 'chip' | 'picture'

function renderZone(flavour: Flavour, zone: ListedZone) {
  const marks = createMarksStore()
  const ids = IDS[zone]
  const table = renderTable({
    fixture,
    ui: (
      <MarksContext value={marks}>
        {flavour === 'chip' ? (
          <ZoneButton zone={zone} owner="Alice" ids={ids} />
        ) : (
          <ZonePicture zone={zone} owner="Alice" ids={ids} width={80} height={112} />
        )}
      </MarksContext>
    ),
  })
  const zoneEl = (): HTMLElement => must(table.container.querySelector<HTMLElement>(`[data-zone="${zone}"]`))
  const mark = (cards: CardMarks, toggles = false): void =>
    act(() => marks.publish(cards, () => undefined, toggles))
  return { ...table, marks, zoneEl, mark }
}

describe.each<Flavour>(['chip', 'picture'])('the %s of a zone', (flavour) => {
  it('has no mark while no card in it is legal', () => {
    const { zoneEl, mark } = renderZone(flavour, 'graveyard')
    mark(markCards([], []))
    expect(zoneEl()).not.toHaveAttribute('data-legal')
    expect(zoneEl().querySelector('[data-slot="legal-badge"]')).toBeNull()
    expect(zoneEl()).toHaveAccessibleName(`Graveyard ${graveyard.length} cards, opens the list`)
  })

  it.each<[ListedZone, number]>([
    ['graveyard', 1],
    ['graveyard', 2],
    ['exile', 1],
  ])('marks the %s with %i card(s) legal, in its ring, badge, name and tooltip', (zone, legal) => {
    const ids = IDS[zone].slice(0, legal)
    const { zoneEl, mark } = renderZone(flavour, zone)
    mark(markCards(IDS[zone], ids))
    expect(zoneEl()).toHaveAttribute('data-legal', String(ids.length))
    expect(zoneEl()).toHaveClass('ring-2', 'ring-primary')
    expect(zoneEl().querySelector('[data-slot="legal-badge"]')).not.toBeNull()
    expect(zoneEl()).toHaveAccessibleName(new RegExp(`${ids.length} can be played or used, opens the list$`))
    expect(zoneEl().getAttribute('title')).toContain(`${ids.length} can be played or used`)
  })

  it('counts only the legal cards, not the dimmed or the selected', () => {
    const [first, second, third] = graveyard as [ObjectId, ObjectId, ObjectId]
    const { zoneEl, mark } = renderZone(flavour, 'graveyard')
    mark(
      new Map([
        [first, 'legal'],
        [second, 'dimmed'],
        [third, 'selected'],
      ]),
    )
    expect(zoneEl()).toHaveAttribute('data-legal', '1')
  })

  it('follows the prompt: the mark goes when the marks do, and comes back with them', () => {
    const { zoneEl, mark, marks } = renderZone(flavour, 'graveyard')
    mark(markCards([], [must(graveyard[0])]))
    expect(zoneEl()).toHaveAttribute('data-legal', '1')
    act(() => marks.clear())
    expect(zoneEl()).not.toHaveAttribute('data-legal')
    mark(markCards([], [must(graveyard[0])]))
    expect(zoneEl()).toHaveAttribute('data-legal', '1')
  })

  it('stays plain for a prompt that picks among choices (marks that toggle)', () => {
    const { zoneEl, mark } = renderZone(flavour, 'graveyard')
    mark(markCards([], [must(graveyard[0])]), true)
    expect(zoneEl()).not.toHaveAttribute('data-legal')
  })

  it('stays plain for the command zone, whose commanders are cast from the hand dock', () => {
    const { zoneEl, mark } = renderZone(flavour, 'command')
    mark(markCards([], commanders))
    expect(zoneEl()).not.toHaveAttribute('data-legal')
  })

  it('stays plain for a card of another zone that is legal', () => {
    const { zoneEl, mark } = renderZone(flavour, 'graveyard')
    mark(markCards([], exile))
    expect(zoneEl()).not.toHaveAttribute('data-legal')
  })

  it('says under the title of the opened list how many can be played or used', async () => {
    const { zoneEl, mark, user } = renderZone(flavour, 'graveyard')
    mark(markCards([], [must(graveyard[0]), must(graveyard[1])]))
    await user.click(zoneEl())
    expect(screen.getByRole('dialog')).toHaveAccessibleDescription(/2 can be played or used\.$/)
  })

  it('has no such sentence in the list of a zone with nothing to play', async () => {
    const { zoneEl, user } = renderZone(flavour, 'graveyard')
    await user.click(zoneEl())
    expect(screen.getByRole('dialog')).toHaveAccessibleDescription(/most recent first\.$/)
  })
})
