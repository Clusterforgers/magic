// Goal: the clause the server marks for a spell's question (the modes as it is cast, a number, a card choice) is marked in the spell's own text under the asking card in the Stack panel, quoted under the question in the dock (the modes not quoted, being the choices), and left out of a phone's bar of fixed height, in a duel and a grid of four.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { must } from '@/test-utils/batches'
import { fixtureGame } from '../../dev/devGame'
import type { FixtureGameOptions } from '../../dev/devGame'
import { GameTable } from '../../GameTable'
import { TableOptionsContext } from '../../tableOptions'

type Viewport = { width: number; height: number }

const MODES: Array<[string, Partial<FixtureGameOptions>]> = [
  ['a duel', { seats: 2, board: 'duel' }],
  ['a grid of four', { seats: 4, permanents: 16 }],
]
const WINDOWS: Array<[string, Viewport, 'panel' | 'dock' | 'bar']> = [
  ['1400x900', { width: 1400, height: 900 }, 'panel'],
  ['a rail at 1100', { width: 1100, height: 800 }, 'dock'],
  ['a phone', { width: 390, height: 844 }, 'bar'],
]
const PROMPTS: Array<[string, string, boolean]> = [
  ['asked-modes', 'Choose one —', false],
  ['asked-number', 'Put up to two +1/+1 counters on target creature.', true],
  ['asked-cards', 'Then put a card from your hand on top of your library.', true],
]

function table(prompt: string, options: Partial<FixtureGameOptions>, viewport: Viewport) {
  const game = fixtureGame({ seats: 2, board: 'duel', ...options, prompt })
  const view = render(
    <GameContext value={game}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return {
    one: (selector: string): HTMLElement => must(view.container.querySelector<HTMLElement>(selector)),
    all: (selector: string): HTMLElement[] => [...view.container.querySelectorAll<HTMLElement>(selector)],
  }
}

describe.each(MODES)('the asked clause of a spell in %s', (_mode, options) => {
  describe.each(PROMPTS)('?prompt=%s', (prompt, clause, quoted) => {
    it.each(WINDOWS)('at %s', (_window, viewport, home) => {
      const shown = table(prompt, options, viewport)
      expect(shown.one('[data-slot="decision-line"]')).toHaveAttribute('data-home', home)
      if (home === 'panel') {
        const marked = shown.one('[data-slot="decision-detail"] [data-slot="asked-clause"]')
        expect(marked.textContent).toContain(clause)
        expect(shown.all('[data-slot="asked-quote"]')).toHaveLength(0)
        return
      }
      const quotes = shown.all('[data-slot="asked-quote"]')
      if (home === 'dock' && quoted) {
        expect(quotes).toHaveLength(1)
        expect(quotes[0]?.textContent).toContain(clause)
        expect(quotes[0]).toHaveAttribute('title', clause)
      } else {
        expect(quotes).toHaveLength(0)
      }
    })
  })
})
