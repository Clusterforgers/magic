// Goal: the card preview draws its picture at the width `previewFit` gives the window (its box set through `--card-w` and `--card-h`), on the side the fit names with the lines under the picture on a tall window and beside it on a short one, keeps the picture sharp at the `normal` size, and keeps its Close button in view while pinned.
import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { PICTURE_FLOOR, previewFit } from './previewFit'

const original = { width: window.innerWidth, height: window.innerHeight }

function resize(width: number, height: number): void {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: width })
  Object.defineProperty(window, 'innerHeight', { configurable: true, value: height })
}

beforeEach(() => resize(1440, 900))
afterEach(() => resize(original.width, original.height))

/** Hovers the first card of the opponent at a box on the screen and gives back the preview and the card box it drew. */
async function hovered(at: DOMRect, pin = false) {
  const { container, user } = renderTable({ seats: 2 })
  const card = must(
    container.querySelector<HTMLElement>('[data-battlefield-seat="1"] button[data-object-id]'),
  )
  card.getBoundingClientRect = () => at
  if (pin) await user.click(card)
  else await user.hover(card)
  const preview = screen.getByTestId('card-preview')
  return { preview, box: must(preview.querySelector<HTMLElement>('div.grid.gap-2')) }
}

describe('the preview picture', () => {
  it.each([
    [1440, 900, new DOMRect(100, 300, 60, 84), 'right'],
    [1440, 900, new DOMRect(1300, 300, 60, 84), 'left'],
    [1920, 1080, new DOMRect(100, 300, 60, 84), 'right'],
    [1280, 720, new DOMRect(100, 300, 60, 84), 'right'],
  ] as const)(
    'is drawn at the fit of a %ix%i window for a card at %j (docked %s)',
    async (w, h, at, dock) => {
      resize(w, h)
      const fit = previewFit(
        { left: at.left, top: at.top, width: at.width, height: at.height },
        { width: w, height: h },
      )
      const { preview, box } = await hovered(at)
      expect(fit.dock).toBe(dock)
      expect(preview).toHaveAttribute('data-dock', dock)
      expect(preview).toHaveAttribute('data-picture', String(fit.picture))
      expect(fit.picture).toBeGreaterThan(PICTURE_FLOOR.side)
      expect(box.style.getPropertyValue('--card-w')).toBe(`${fit.picture}px`)
      expect(box.style.getPropertyValue('--card-h')).toBe(`${(fit.picture * 7) / 5}px`)
      expect(preview.style.width).toBe(`${fit.width}px`)
      expect(preview.style.maxHeight).toBe(`${fit.maxHeight}px`)
      expect(preview.style.top).toBe('64px')
    },
  )

  it('stands the lines under the picture, which is centred, on a phone, above the hand dock for a card at the top', async () => {
    resize(390, 844)
    const { preview, box } = await hovered(new DOMRect(20, 100, 60, 84))
    expect(preview).toHaveAttribute('data-dock', 'bottom')
    expect(preview).not.toHaveClass('flex')
    expect(preview.style.bottom).toBe('126px')
    expect(Number(preview.dataset.picture)).toBeGreaterThan(PICTURE_FLOOR.narrow)
    expect(must(box.parentElement)).toHaveClass('mx-auto')
  })

  it('keeps the old size with the text beside the picture in a narrow, short window', async () => {
    resize(568, 320)
    const { preview } = await hovered(new DOMRect(20, 100, 60, 84))
    expect(preview).toHaveClass('flex')
    expect(preview).toHaveAttribute('data-picture', String(PICTURE_FLOOR.narrow))
  })

  it('asks for the normal picture at any width, never the large one', async () => {
    const { preview } = await hovered(new DOMRect(100, 300, 60, 84))
    const sources = [...preview.querySelectorAll('img')].map((image) => image.getAttribute('src') ?? '')
    expect(sources.every((src) => src.endsWith('/normal'))).toBe(true)
  })

  it('keeps Close preview at the foot of the panel while the lines above scroll', async () => {
    const { preview } = await hovered(new DOMRect(100, 300, 60, 84), true)
    const close = must(screen.getByRole('button', { name: 'Close preview' }).parentElement)
    expect(close).toHaveClass('sticky')
    expect(preview).toContainElement(close)
    expect(preview).toHaveClass('overflow-y-auto')
  })
})
