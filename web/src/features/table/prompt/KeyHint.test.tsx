// Goal: a key hint beside an answer is decoration for a keyboard: hidden from a reader and left out on a touch screen.
import { render } from '@testing-library/react'
import { expect, it } from 'vitest'
import { KeyHint } from './KeyHint'

it('draws the key for a keyboard only', () => {
  const { container } = render(<KeyHint>Esc</KeyHint>)
  const hint = container.querySelector('[data-slot="key-hint"]')
  expect(hint).toHaveTextContent('Esc')
  expect(hint).toHaveAttribute('aria-hidden', 'true')
  expect(hint).toHaveClass('pointer-coarse:hidden')
})
