// Goal: the preview of a card gives one line of reminder text for each keyword the card has or its ability texts mention, and none for a card with no keyword.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'

const fixture = buildFixture({ seats: 2 })
const mine = must(
  fixture.view.objects.find(
    (object) => object.zone === 'Battlefield' && object.controller === 0 && object.types.includes('Creature'),
  ),
)

async function hover(change: (object: ObjectView) => void) {
  const table = renderTable({
    fixture,
    state: (state: PlayState) => {
      const view = clone(must(state.view))
      const shown = must(view.objects.find((o) => o.id === mine.id))
      change(shown)
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
  await table.user.hover(must(table.container.querySelector(`button[data-object-id="${mine.id}"]`)))
  return screen.getByTestId('card-preview').querySelector('[data-slot="preview-reminders"]')
}

describe('the keyword reminders in the preview', () => {
  it('lists a keyword of the card and one its activated text mentions', async () => {
    const list = await hover((object) => {
      object.keywords = ['Ward', 'First strike']
      object.activated = [{ cost: '{1}', effect: 'Discover 4.' }]
    })
    expect(list).toHaveAccessibleName('Keyword reminders')
    const lines = [...(list?.querySelectorAll('li') ?? [])].map((item) => item.textContent ?? '')
    expect(lines.map((line) => line.split(':')[0])).toEqual(['Ward', 'First strike', 'Discover'])
    expect(lines[2]).toContain('Exile cards from the top of your library')
  })

  it('lists none for a card without a keyword', async () => {
    const list = await hover((object) => {
      object.keywords = []
      object.abilities = []
      object.activated = []
      object.manual = []
    })
    expect(list).toBeNull()
  })
})
