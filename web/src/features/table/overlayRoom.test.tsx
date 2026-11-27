// Goal: a face too narrow for the smallest head mark (the 16 and 20 px pictures of a zone, stack or top-card chip) is a picture alone, so no mark is cut off by its corner; a face of the board's narrowest fit and wider keeps its marks.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { card } from '@/test-utils/cards'
import { MARKS_MIN_PX, Overlay } from './CardOverlay'
import { FIT_LIMITS } from './fit'

function marks(width: number) {
  const object = card({ read: true })
  const { container } = render(
    <Overlay object={object} role={null} size="compact" turned={false} width={width} />,
  )
  return container.querySelector('[data-slot="card-read"]')
}

describe('Overlay room', () => {
  it('keeps the narrowest face that draws marks below the board fit', () => {
    expect(MARKS_MIN_PX).toBeLessThanOrEqual(FIT_LIMITS.minWidth)
  })

  it.each([
    [16, false],
    [20, false],
    [MARKS_MIN_PX - 1, false],
    [MARKS_MIN_PX, true],
    [FIT_LIMITS.minWidth, true],
    [60, true],
  ])('a face %i px wide draws its marks: %s', (width, drawn) => {
    expect(marks(width) !== null).toBe(drawn)
  })
})
