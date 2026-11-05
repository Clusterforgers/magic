// Goal: a seat that the Ring tempted shows a chip with its level and, in words, its Ring-bearer or that it has none; a seat the Ring has not tempted shows no chip.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { PlayState } from '@/play/store'
import type { StateView } from '@/protocol/generated'
import { buildFixture } from './dev/fixture'
import { ringWords } from './ring'
import { SeatPanel } from './SeatPanel'
import { CombatProviders } from './combat/CombatProviders'

function edited(edit: (view: StateView) => void): (state: PlayState) => PlayState {
  return (state) => {
    const view = clone(must(state.view))
    edit(view)
    return { ...state, view }
  }
}

function show(edit: (view: StateView) => void = () => undefined) {
  return renderTable({
    fixture: buildFixture({ seats: 3, permanents: 6 }),
    ui: (
      <CombatProviders>
        <SeatPanel seat={1} />
      </CombatProviders>
    ),
    state: edited(edit),
  })
}

describe('ringWords', () => {
  it.each([
    [2, 'Elf', "Bo's Ring: level 2, Ring-bearer Elf"],
    [1, undefined, "Bo's Ring: level 1, no Ring-bearer"],
  ])('says level %s with bearer %s', (level, bearer, text) => {
    expect(ringWords('Bo', level, bearer)).toBe(text)
  })
})

describe('the Ring chip', () => {
  it('shows the level of a tempted seat', () => {
    show((view) => {
      must(view.players[1]).ring_level = 3
    })
    expect(screen.getByRole('img', { name: /Ring: level 3, no Ring-bearer/ })).toHaveTextContent('3')
  })

  it('is left out while the Ring has not tempted the seat', () => {
    show()
    expect(document.querySelector('[data-slot="ring"]')).toBeNull()
  })
})
