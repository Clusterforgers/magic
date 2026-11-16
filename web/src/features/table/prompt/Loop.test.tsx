/** Goal: in every table layout (grid, focus, the boss table) at two to four seats, the Loop prompt names the loop's cards and what one iteration changes, repeats it 10, 100 or 1,000 times or a typed number up to its cap, refuses a number out of range with the reason, stops it with Stop or S, offers the draw only for a loop that never ends by itself, and starts on the answer the response clock gives. */
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { LoopOptions } from '@/protocol/generated'
import type { TableMode } from '../layout'
import { renderDecision } from '@/test-utils/combatTable'

const polyraptor: LoopOptions = {
  cards: ['Polyraptor', 'Where Ancients Tread'],
  per_iteration: {
    tokens: [{ controller: 0, name: 'Polyraptor', count: 1 }],
    life: [{ player: 1, change: -5 }],
  },
  mandatory: false,
  bounded: false,
  max: 10_000,
  asked: 0,
  can_draw: false,
}

function mount(given: Partial<LoopOptions> = {}, seats = 2, mode?: TableMode) {
  return renderDecision(
    { Loop: { ...polyraptor, ...given } },
    { seats, options: mode === undefined ? {} : { mode } },
  )
}

describe.each<[number, TableMode | undefined]>([
  [2, undefined],
  [3, 'grid'],
  [4, 'focus'],
  [4, 'boss'],
])('at %i seats in the %s layout', (seats, mode) => {
  it('names the cards and what one iteration does', () => {
    const { text } = mount({}, seats, mode)
    expect(text()).toContain('Polyraptor and Where Ancients Tread make a loop.')
    expect(text()).toContain('Each time: +1 Polyraptor token for you')
    expect(text()).toContain('loses 5 life')
  })

  it.each([10, 100, 1000])('repeats it %i times from a preset', async (times) => {
    const { answer, user } = mount({}, seats, mode)
    await user.click(screen.getByRole('button', { name: `Repeat ${times.toLocaleString('en-US')} times` }))
    expect(answer).toHaveBeenCalledWith({ Loop: { Repeat: times } })
  })
})

describe('the number field', () => {
  it('repeats a typed number', async () => {
    const { answer, user } = mount()
    await user.type(screen.getByRole('spinbutton', { name: 'Times to repeat' }), '250{Enter}')
    expect(answer).toHaveBeenCalledWith({ Loop: { Repeat: 250 } })
  })

  it.each([
    ['0', 'At least 1'],
    ['20000', 'At most 10,000'],
  ])('refuses %s with the reason', async (typed, reason) => {
    const { answer, user } = mount()
    await user.type(screen.getByRole('spinbutton', { name: 'Times to repeat' }), typed)
    expect(screen.getByRole('alert')).toHaveTextContent(reason)
    expect(screen.getByRole('button', { name: 'Repeat' })).toBeDisabled()
    await user.keyboard('{Enter}')
    expect(answer).not.toHaveBeenCalled()
  })

  it('turns off the presets past the cap', () => {
    mount({ max: 50 })
    expect(screen.getByRole('button', { name: 'Repeat 10 times' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Repeat 100 times' })).toBeDisabled()
  })
})

describe('stopping and the draw', () => {
  it('stops with the button, which has the focus, and with S', async () => {
    const { answer, user } = mount()
    expect(screen.getByRole('button', { name: 'Stop the loop' })).toHaveFocus()
    await user.keyboard('s')
    expect(answer).toHaveBeenCalledWith({ Loop: 'Stop' })
  })

  it('offers no draw for a loop someone drives', () => {
    mount()
    expect(screen.queryByRole('button', { name: /game is a draw/ })).toBeNull()
  })

  it('offers the draw, focused, for a loop that runs by itself forever', async () => {
    const { answer, text, user } = mount({
      mandatory: true,
      can_draw: true,
      cards: ['Polyraptor', 'Aether Flash'],
    })
    expect(text()).toContain('Polyraptor and Aether Flash repeat by themselves.')
    const draw = screen.getByRole('button', { name: 'Let it loop: the game is a draw' })
    expect(draw).toHaveFocus()
    await user.click(draw)
    expect(answer).toHaveBeenCalledWith({ Loop: 'Draw' })
  })
})
