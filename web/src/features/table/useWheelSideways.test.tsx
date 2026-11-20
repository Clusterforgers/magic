// Goal: a wheel turned over a line that scrolls sideways only moves the line by the turn (px, lines or pages), and leaves the turn alone where the line does not scroll sideways, scrolls up and down itself, the turn is sideways already, a zoom is asked for, or the line is at the end the turn moves to.
import { fireEvent, render } from '@testing-library/react'
import { useRef } from 'react'
import { describe, expect, it } from 'vitest'
import { sidewaysStep, useWheelSideways } from './useWheelSideways'

const LINE = { scrollLeft: 100, scrollWidth: 900, clientWidth: 300, upDown: false }

describe('sidewaysStep', () => {
  it.each([
    { name: 'a turn down moves right', line: LINE, wheel: { deltaX: 0, deltaY: 40, deltaMode: 0 }, step: 40 },
    { name: 'a turn up moves left', line: LINE, wheel: { deltaX: 0, deltaY: -40, deltaMode: 0 }, step: -40 },
    { name: 'lines count 16 px', line: LINE, wheel: { deltaX: 0, deltaY: 3, deltaMode: 1 }, step: 48 },
    { name: 'a page is the line wide', line: LINE, wheel: { deltaX: 0, deltaY: 1, deltaMode: 2 }, step: 300 },
    {
      name: 'a line that fits',
      line: { ...LINE, scrollWidth: 300 },
      wheel: { deltaX: 0, deltaY: 40, deltaMode: 0 },
      step: 0,
    },
    {
      name: 'a line that scrolls up and down',
      line: { ...LINE, upDown: true },
      wheel: { deltaX: 0, deltaY: 40, deltaMode: 0 },
      step: 0,
    },
    { name: 'a sideways turn', line: LINE, wheel: { deltaX: 40, deltaY: 10, deltaMode: 0 }, step: 0 },
    {
      name: 'up at the start',
      line: { ...LINE, scrollLeft: 0 },
      wheel: { deltaX: 0, deltaY: -40, deltaMode: 0 },
      step: 0,
    },
    {
      name: 'down at the end',
      line: { ...LINE, scrollLeft: 600 },
      wheel: { deltaX: 0, deltaY: 40, deltaMode: 0 },
      step: 0,
    },
  ])('$name', ({ line, wheel, step }) => {
    expect(sidewaysStep(line, wheel)).toBe(step)
  })
})

function Line() {
  const ref = useRef<HTMLUListElement>(null)
  useWheelSideways(ref)
  return <ul ref={ref} data-testid="line" style={{ overflowX: 'auto', overflowY: 'hidden' }} />
}

/** The line, with the scroll metrics jsdom does not lay out. */
function line(): HTMLElement {
  const { getByTestId } = render(<Line />)
  const element = getByTestId('line')
  Object.defineProperty(element, 'scrollWidth', { value: 900 })
  Object.defineProperty(element, 'clientWidth', { value: 300 })
  Object.defineProperty(element, 'scrollHeight', { value: 200 })
  Object.defineProperty(element, 'clientHeight', { value: 150 })
  return element
}

describe('useWheelSideways', () => {
  it('moves the line by a turn of the wheel and keeps the turn from scrolling anything else', () => {
    const element = line()
    const unhandled = fireEvent.wheel(element, { deltaY: 120 })
    expect(element.scrollLeft).toBe(120)
    expect(unhandled).toBe(false)
  })

  it('lets a zoom and a sideways turn pass', () => {
    const element = line()
    expect(fireEvent.wheel(element, { deltaY: 120, ctrlKey: true })).toBe(true)
    expect(fireEvent.wheel(element, { deltaX: 120 })).toBe(true)
    expect(element.scrollLeft).toBe(0)
  })

  it('moves the line for a turn over one of its cards', () => {
    const element = line()
    const card = document.createElement('li')
    element.append(card)
    fireEvent.wheel(card, { deltaY: 60 })
    expect(element.scrollLeft).toBe(60)
  })
})
