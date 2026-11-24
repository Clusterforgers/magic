// Goal: what changes a player or a card is named by source: a seat's effect chip ends its sentence with the objects that bring it (`PlayerView.sources`) in the panel, the one-line pod and the tile, each name once, and stays as it was while none does; the preview's "Modified by" lists a card's cost changes (a tax, a reduction, the drawn colours) with their sources at two and four seats.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import type { ModifierView, PlayerEffect, PlayerEffectSourceView, PlayerView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { CombatProviders } from './combat/CombatProviders'
import { buildFixture } from './dev/fixture'
import { effectSources, seatEffects } from './playerEffects'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'

const VORTEX: PlayerEffectSourceView = { effect: 'CantGainLife', source: 'Sulfuric Vortex', id: 40 }
const PULSE: PlayerEffectSourceView = { effect: 'CantGainLife', source: 'Erebos', id: 41 }

const player = (effects: PlayerEffect[], sources: PlayerEffectSourceView[]): PlayerView =>
  ({ ...must(buildFixture({ seats: 2 }).view.players[1]), effects, sources }) as PlayerView

describe('the sources of an effect on a player', () => {
  it.each<[string, PlayerEffectSourceView[], string]>([
    ['one source', [VORTEX], "Bo can't gain life. From Sulfuric Vortex."],
    ['two sources', [VORTEX, PULSE], "Bo can't gain life. From Sulfuric Vortex and Erebos."],
    ['one source twice', [VORTEX, VORTEX], "Bo can't gain life. From Sulfuric Vortex."],
    ['no source', [], "Bo can't gain life."],
  ])('ends the sentence with %s', (_what, sources, detail) => {
    const [facts] = seatEffects(player(['CantGainLife'], sources), {}, 'Bo')
    expect(facts?.detail).toBe(detail)
  })

  it('matches an effect with a value by its value', () => {
    const sources: PlayerEffectSourceView[] = [
      { effect: { MaxHandSize: 4 }, source: 'Spirit of the Labyrinth' },
      { effect: { MaxHandSize: 5 }, source: 'Other' },
    ]
    expect(effectSources({ MaxHandSize: 4 }, player([{ MaxHandSize: 4 }], sources))).toEqual([
      'Spirit of the Labyrinth',
    ])
  })

  it.each(['panel', 'line', 'tile'] as const)('names the source in the %s pod', (place) => {
    const ui =
      place === 'tile' ? (
        <SeatTile seat={1} focused={false} onFocusSeat={() => undefined} />
      ) : (
        <SeatPanel seat={1} variant={place} />
      )
    renderTable({
      fixture: buildFixture({ seats: 3, permanents: 6 }),
      ui: <CombatProviders>{ui}</CombatProviders>,
      state: (state: PlayState) => {
        const view = clone(must(state.view))
        must(view.players[1]).effects = ['CantGainLife']
        must(view.players[1]).sources = [VORTEX]
        return { ...state, view }
      },
    })
    const named = /can't gain life\. From Sulfuric Vortex\./
    const found =
      place === 'tile'
        ? must(document.querySelector<HTMLElement>('[data-slot="player-effects"]')).getAttribute('title')
        : screen.getByRole('img', { name: named }).getAttribute('aria-label')
    expect(found).toMatch(named)
  })
})

describe('the cost changes of a card in the preview', () => {
  const COSTS: ModifierView[] = [
    { effect: 'Costs {1} more', source: 'Thalia, Guardian of Thraben', count: 1 },
    { effect: 'Costs {1} less', source: 'Goblin Electromancer', count: 1 },
    { effect: 'Costs {1}{W}{U}{U} this round', source: 'The Unmaker', count: 1 },
  ]

  it.each([2, 4])('lists each change with its source at %i seats', async (seats) => {
    const fixture = buildFixture({ seats })
    const card = must(fixture.view.objects.find((object) => object.zone === 'Battlefield'))
    const { user, container } = renderTable({
      fixture,
      state: (state: PlayState) => ({
        ...state,
        objectsById: { ...state.objectsById, [card.id]: { ...card, modifiers: COSTS } },
      }),
    })
    await user.hover(must(container.querySelector<HTMLElement>(`button[data-object-id="${card.id}"]`)))
    const list = within(screen.getByTestId('card-preview')).getByRole('list', { name: 'Modified by' })
    const lines = within(list).getAllByRole('listitem')
    expect(lines).toHaveLength(COSTS.length)
    COSTS.forEach(({ effect, source }, index) => {
      // Said as "effect from source", drawn as "effect · source".
      expect(lines[index]).toHaveTextContent(`${effect} from ${source}`)
      expect(lines[index]).toHaveTextContent(`${effect} · ${source}`)
    })
  })
})
