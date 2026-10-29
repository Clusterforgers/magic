// Goal: outside the table a card's picture comes from the server's endpoint by oracle id in a reserved 5:7 box, a mouse hover shows it beside the name while touch does not, and Text only draws none.
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { CardPicture, HoverPicture } from './CardPicture'
import { resetImageChoice, setImageChoice } from './imageSetting'

afterEach(() => {
  resetImageChoice()
  window.localStorage.clear()
})

describe('CardPicture', () => {
  it.each<[number, string]>([
    [0, '/api/images/oracle/bolt/0/normal'],
    [1, '/api/images/oracle/bolt/1/normal'],
  ])('face %i', (face, src) => {
    const { container } = render(<CardPicture oracleId="bolt" name="Lightning Bolt" face={face} />)
    expect(container.querySelector('img')).toHaveAttribute('src', src)
    expect(container.querySelector('[data-slot="card-image"]')).toHaveClass('aspect-[5/7]')
  })

  it('draws nothing with Text only', () => {
    setImageChoice('text')
    const { container } = render(<CardPicture oracleId="bolt" name="Lightning Bolt" />)
    expect(container.querySelector('img')).toBeNull()
  })
})

describe('HoverPicture', () => {
  it.each<[string, boolean]>([
    ['mouse', true],
    ['touch', false],
  ])('a %s hover shows the picture: %s', (pointerType, shown) => {
    const { container } = render(
      <HoverPicture oracleId="bolt" name="Lightning Bolt">
        <button type="button">Lightning Bolt</button>
      </HoverPicture>,
    )
    act(() => fireEvent.pointerEnter(screen.getByRole('button'), { pointerType }))
    expect(container.querySelector('[data-slot="hover-picture"]') !== null).toBe(shown)
    act(() => fireEvent.pointerLeave(screen.getByRole('button'), { pointerType }))
    expect(container.querySelector('[data-slot="hover-picture"]')).toBeNull()
  })
})
