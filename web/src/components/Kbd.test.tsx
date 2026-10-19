// Goal: a key hint is a real kbd element that carries its key name, takes the colour of the control it sits in and accepts extra classes.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Kbd } from './Kbd'

describe('Kbd', () => {
  it('is a kbd element with the key printed on it', () => {
    render(<Kbd>Space</Kbd>)
    const key = screen.getByText('Space')
    expect(key.tagName).toBe('KBD')
    expect(key).toHaveAttribute('data-slot', 'kbd')
  })

  it('draws its edge in the colour of the text around it', () => {
    render(<Kbd>Esc</Kbd>)
    expect(screen.getByText('Esc')).toHaveClass('border-current/30')
  })

  it('keeps the class names and attributes it is given', () => {
    render(
      <Kbd className="ml-2" title="Escape">
        Esc
      </Kbd>,
    )
    expect(screen.getByText('Esc')).toHaveClass('ml-2', 'inline-flex')
    expect(screen.getByText('Esc')).toHaveAttribute('title', 'Escape')
  })
})
