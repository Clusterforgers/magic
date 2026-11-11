// Goal: the effects on a player are chips at its seat in the panel and the one-line pod, each an icon and words with the sentence on hover or focus, and one figure on a tile whose spoken name lists them; a seat without effects draws none.
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import type { PlayerEffect } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { CombatProviders } from './combat/CombatProviders'
import { buildFixture } from './dev/fixture'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'

const EFFECTS: PlayerEffect[] = ['CantLose', { Keyword: 'Hexproof' }, { ExtraLandPlays: 1 }]

function show(place: 'panel' | 'line' | 'tile', effects: PlayerEffect[]) {
  const ui =
    place === 'tile' ? (
      <SeatTile seat={1} focused={false} onFocusSeat={() => undefined} />
    ) : (
      <SeatPanel seat={1} variant={place} />
    )
  return renderTable({
    fixture: buildFixture({ seats: 3, permanents: 6 }),
    ui: <CombatProviders>{ui}</CombatProviders>,
    state: (state: PlayState) => {
      const view = clone(must(state.view))
      must(view.players[1]).effects = effects
      return { ...state, view }
    },
  })
}

const chips = () => [...document.querySelectorAll<HTMLElement>('[data-slot="player-effect"]')]

describe('PlayerEffectChips', () => {
  it.each(['panel', 'line'] as const)('draws a chip per effect in the %s pod, words and all', (place) => {
    show(place, EFFECTS)
    expect(chips().map((chip) => chip.textContent)).toEqual(["Can't lose", 'Hexproof', '+1 land'])
    expect(screen.getByRole('img', { name: "Bo can't lose the game." })).toBeInTheDocument()
    expect(chips().every((chip) => chip.dataset.tone !== undefined)).toBe(true)
  })

  it('says what an effect does on focus', async () => {
    show('panel', EFFECTS)
    act(() => must(chips()[0]).focus())
    expect(
      await screen.findByText("Bo can't lose the game.", {
        selector: '[data-slot=tooltip-content] *, [data-slot=tooltip-content]',
      }),
    ).toBeInTheDocument()
  })

  it('draws one figure on a tile and names the effects in the tile', () => {
    show('tile', EFFECTS)
    expect(chips()).toEqual([])
    expect(document.querySelector('[data-slot="player-effects"]')).toHaveTextContent('3')
    expect(screen.getByRole('button', { name: /Show Bo's board/ })).toHaveAccessibleName(
      expect.stringContaining("Can't lose, Hexproof, +1 land"),
    )
  })

  it.each(['panel', 'line', 'tile'] as const)(
    'draws nothing in the %s for a seat without effects',
    (place) => {
      show(place, [])
      expect(chips()).toEqual([])
      expect(document.querySelector('[data-slot="player-effects"]')).toBeNull()
    },
  )
})
