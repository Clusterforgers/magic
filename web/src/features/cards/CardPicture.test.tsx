// Goal: outside the table a card's picture comes from the server's endpoint by oracle id in a reserved 5:7 box, a mouse hover shows it large in a panel docked as the table preview is, with its type and support, while touch does not, and Text only draws none.
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { CardPicture, HoverPicture } from './CardPicture'
import type { HoverInfo } from './CardPicture'
import { hoverFit, HOVER_PADDING_PX } from './hoverFit'
import { hoverPreview } from './hoverPreview'
import { resetImageChoice, setImageChoice } from './imageSetting'

afterEach(() => {
  act(() => hoverPreview.dismiss())
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
    render(
      <HoverPicture oracleId="bolt" name="Lightning Bolt">
        <button type="button">Lightning Bolt</button>
      </HoverPicture>,
    )
    act(() => fireEvent.pointerEnter(screen.getByRole('button'), { pointerType }))
    expect(document.querySelector('[data-slot="hover-picture"]') !== null).toBe(shown)
    act(() => fireEvent.pointerLeave(screen.getByRole('button'), { pointerType }))
    expect(document.querySelector('[data-slot="hover-picture"]')).toBeNull()
  })

  it('docks the panel at the width the window allows, as the table preview does', () => {
    render(
      <HoverPicture oracleId="bolt" name="Lightning Bolt">
        <button type="button">Lightning Bolt</button>
      </HoverPicture>,
    )
    act(() => fireEvent.pointerEnter(screen.getByRole('button'), { pointerType: 'mouse' }))
    const shown = document.querySelector<HTMLElement>('[data-slot="hover-picture"]')
    const fit = hoverFit({ left: 0, right: 0, top: 0, bottom: 0 }, { width: innerWidth, height: innerHeight })
    expect(shown?.style.width).toBe(`${fit.width + HOVER_PADDING_PX}px`)
    expect(shown).toHaveAttribute('data-dock', fit.dock)
  })

  it.each<[string, HoverInfo, readonly string[], readonly string[]]>([
    ['a Verified card', { typeLine: 'Instant', support: 'verified' }, ['Instant', 'Verified'], ['×']],
    [
      'a card played by hand, with copies and reasons',
      { typeLine: 'Artifact', count: 3, support: 'manual', reasons: ['No rules yet', 'No rules yet'] },
      ['×3', 'Artifact', 'Manual', 'No rules yet'],
      [],
    ],
  ])('says under the picture what it knows of %s', (_, info, shown, absent) => {
    render(
      <HoverPicture oracleId="bolt" name="Lightning Bolt" info={info}>
        <button type="button">Lightning Bolt</button>
      </HoverPicture>,
    )
    act(() => fireEvent.pointerEnter(screen.getByRole('button'), { pointerType: 'mouse' }))
    const panel = document.querySelector<HTMLElement>('[data-slot="hover-picture"]') as HTMLElement
    for (const text of shown) expect(panel).toHaveTextContent(text)
    for (const text of absent) expect(panel).not.toHaveTextContent(text)
    expect(panel.textContent?.split('No rules yet').length ?? 0).toBeLessThanOrEqual(2)
  })
})
