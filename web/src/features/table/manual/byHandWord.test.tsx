// Goal: the "By hand" mark at the head of a picture keeps its word only where the head has room for it, and is the hand icon alone on a narrower or compact card, so it is never cut by the card's edge.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ManualClause } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { Overlay } from '../CardOverlay'

const ONE: ManualClause[] = [{ text: 'Flip a coin.', reason: 'a random choice' }]

describe('the By hand mark of a picture', () => {
  it.each<[number, boolean, 'normal' | 'compact', boolean]>([
    [150, false, 'normal', true],
    [120, false, 'normal', true],
    [100, false, 'normal', true],
    [99, false, 'normal', false],
    [80, false, 'normal', false],
    [150, true, 'normal', true],
    [90, true, 'normal', false],
    [150, false, 'compact', false],
  ])('on a %i px card (turned %s, %s) shows its word: %s', (width, turned, size, word) => {
    const { container } = render(
      <Overlay object={card({ manual: ONE })} role={null} size={size} turned={turned} width={width} />,
    )
    const mark = must(container.querySelector('[data-slot="card-support"]'))
    expect(mark.textContent).toBe(word ? 'By hand' : '')
  })
})
