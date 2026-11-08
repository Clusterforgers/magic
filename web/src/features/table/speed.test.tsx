// Goal: a seat with speed shows a chip with the number, marked and worded as max speed at 4; a seat without speed shows no chip.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { PlayState } from '@/play/store'
import type { StateView } from '@/protocol/generated'
import { buildFixture } from './dev/fixture'
import { speedWords } from './speed'
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

describe('speedWords', () => {
  it.each([
    [1, "Bo's speed: 1"],
    [3, "Bo's speed: 3"],
    [4, "Bo's speed: 4, max speed"],
  ])('says speed %s', (speed, text) => {
    expect(speedWords('Bo', speed)).toBe(text)
  })
})

describe('the speed chip', () => {
  it('shows the speed of a seat that has one', () => {
    show((view) => {
      must(view.players[1]).speed = 2
    })
    const chip = screen.getByRole('img', { name: /speed: 2$/ })
    expect(chip).toHaveTextContent('2')
    expect(chip).not.toHaveAttribute('data-max')
  })

  it('marks max speed', () => {
    show((view) => {
      must(view.players[1]).speed = 4
    })
    expect(screen.getByRole('img', { name: /speed: 4, max speed/ })).toHaveAttribute('data-max', 'true')
  })

  it('is left out while the seat has no speed', () => {
    show()
    expect(document.querySelector('[data-slot="speed"]')).toBeNull()
  })
})
