// Goal: a compact face narrower than NARROW_PX draws its name on one line cut by an ellipsis; its marks are glyphs (narrowMarks.test.tsx).
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { permanent } from '@/test-utils/cards'
import { TextFace } from './CardFace'
import { NARROW_PX } from './cardSize'
import { CardWidthContext } from './ImageFace'

function draw(width: number) {
  const object = permanent({ summoning_sick: true }, { name: 'Bonesplitter of the Deep' })
  return render(
    <CardWidthContext.Provider value={width}>
      <TextFace object={object} role={null} size="compact" fit />
    </CardWidthContext.Provider>,
  ).container
}

describe('narrow compact face', () => {
  it.each([60, NARROW_PX - 1])('draws a one-line name at %i px', (width) => {
    const box = draw(width)
    expect(box.querySelector('[data-name]')?.className).toContain('line-clamp-1')
    expect(box.querySelector('[data-name] .truncate')?.textContent).toBe('Bonesplitter of the Deep')
  })

  it('keeps the word breaks at the narrow limit', () => {
    const box = draw(NARROW_PX)
    expect(box.querySelector('[data-name]')?.className).not.toContain('line-clamp-1')
  })
})
