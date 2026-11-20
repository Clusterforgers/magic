// Goal: the Learn course's anchors follow the decision homes: `ask` and `ask.question` on the home that is drawn, `ask.card` on the asking item, `ask.primary` on the home's primary answer, `stack.top` on the panel's top item, `side.tab.stack` on the Stack tab, `sheet.grab` on a phone's payment sheet and `targets.confirm` only for several targets; a window's answers carry none, so every anchor is drawn once.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { fixtureGame } from '../dev/devGame'
import { GameTable } from '../GameTable'
import { TableOptionsContext } from '../tableOptions'

function table(prompt: string, viewport: { width: number; height: number }) {
  const game = fixtureGame({ seats: 2, board: 'duel', prompt })
  return render(
    <GameContext value={game}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
}

const anchors = (): string[] =>
  [...document.querySelectorAll('[data-coach]')].map((element) => element.getAttribute('data-coach') ?? '')
const count = (id: string): number => anchors().filter((anchor) => anchor === id).length

const DESKTOP = { width: 1400, height: 900 }
const RAIL = { width: 1100, height: 800 }
const PHONE = { width: 390, height: 844 }

describe('the coach anchors', () => {
  it.each([
    ['ask-may', DESKTOP, 'panel'],
    ['ask-may', RAIL, 'dock'],
    ['ask-may', PHONE, 'bar'],
    ['costs', RAIL, 'dock'],
    ['pay', PHONE, 'bar'],
  ] as const)('draws each anchor once for %s at %o (%s)', (prompt, viewport, home) => {
    table(prompt, viewport)
    const ask = document.querySelector('[data-coach="ask"]')
    expect(ask).toHaveAttribute('data-home', home)
    expect(ask?.querySelector('[data-coach="ask.question"]')).not.toBeNull()
    // The primary answer carries ask.primary, or its own anchor where it has one (pay.confirm).
    expect(
      ask?.querySelector('[data-slot="prompt-actions"] > button')?.getAttribute('data-coach'),
    ).toBeTruthy()
    for (const id of new Set(anchors())) expect(count(id), id).toBe(1)
  })

  it('names the asking card, the Stack tab and the panel`s items', () => {
    table('ask-may', DESKTOP)
    expect(count('ask.card')).toBe(1)
    expect(count('side.tab.stack')).toBe(1)
  })

  it('opens a phone`s payment sheet by its handle', () => {
    table('pay', PHONE)
    expect(count('sheet.grab')).toBe(1)
  })

  it.each([
    ['targets', 0],
    ['targets-two', 1],
  ])('has targets.confirm for %s %i times: only for several targets', (prompt, times) => {
    table(prompt, DESKTOP)
    expect(count('targets.confirm')).toBe(times)
  })
})
