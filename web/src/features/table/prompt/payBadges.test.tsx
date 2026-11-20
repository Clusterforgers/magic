// Goal: a payment lays nothing out until "Pay for me" is pressed; then the sources it proposes draw dashed and turned with "pays U" on the board, and a change by hand takes those marks away; `paysWords` names what a source pays.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { ManaOption } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { TEST_VIEWPORT } from '@/test-utils/tableHarness'
import { fixtureGame } from '../dev/devGame'
import { GameTable } from '../GameTable'
import { TableOptionsContext } from '../tableOptions'
import { paysWords } from './payText'

function table() {
  const game = fixtureGame({ seats: 2, prompt: 'pay-sources' })
  render(
    <GameContext value={game}>
      <TableOptionsContext value={{ viewport: TEST_VIEWPORT }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return userEvent.setup()
}

const marks = (): string => document.querySelector('[data-slot="ask-marks"]')?.textContent ?? ''

describe('the proposed payment on the board', () => {
  it('lays nothing out until Pay for me, then marks its sources dashed and turned with what they pay', async () => {
    const user = table()
    expect(marks()).not.toContain('pays ')
    await user.click(screen.getByRole('button', { name: 'Pay for me' }))
    expect(marks()).toMatch(/content:'pays [WUBRGC]+'/)
    expect(marks()).toContain('dashed')
    expect(marks()).toContain('rotate:-8deg')
    await user.click(must(screen.getAllByRole('button', { name: 'Undo the last choice' })[0]))
    expect(marks()).not.toContain('pays ')
  })
})

describe('paysWords', () => {
  const option = (produces: ManaOption['produces'], count?: number): ManaOption =>
    ({ source: { object: 1, ability: 0 }, produces, ...(count === undefined ? {} : { count }) }) as ManaOption
  it.each([
    ['Blue', undefined, 'pays U'],
    ['Green', 2, 'pays GG'],
    ['Colorless', 1, 'pays C'],
  ] as const)('names %s ×%s as %s', (produces, count, words) => {
    expect(paysWords(option(produces, count))).toBe(words)
  })
})
