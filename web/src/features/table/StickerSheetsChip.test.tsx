// Goal: the sticker chip of a seat names its sheets and the stickers left on them, lists every sticker with the used ones struck through on hover, and draws nothing for a seat without sheets.
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { TooltipProvider } from '@/components/ui/tooltip'
import type { PlayerView, StickerSheetView } from '@/protocol/generated'
import { StickerSheetsChip, stickerSheetsWords } from './StickerSheetsChip'

const SHEETS: StickerSheetView[] = [
  {
    name: 'Fun Sheet',
    stickers: [
      { text: '“Wicked” ({TK})', tickets: 1, used: true },
      { text: '5/5 ({TK}{TK}{TK})', tickets: 3, used: false },
    ],
  },
  { name: 'Other Sheet', stickers: [{ text: 'Flying ({TK}{TK})', tickets: 2, used: false }] },
]

function draw(sheets: StickerSheetView[] | undefined) {
  const player = { sticker_sheets: sheets } as unknown as PlayerView
  return render(
    <TooltipProvider delay={0}>
      <StickerSheetsChip player={player} />
    </TooltipProvider>,
  )
}

describe('StickerSheetsChip', () => {
  it.each([
    [[SHEETS[1]!], '1 sticker sheet, 1 sticker left: Other Sheet'],
    [SHEETS, '2 sticker sheets, 2 stickers left: Fun Sheet, Other Sheet'],
  ])('names the sheets and the stickers left', (sheets, words) => {
    expect(stickerSheetsWords(sheets)).toBe(words)
    draw(sheets)
    expect(screen.getByRole('img', { name: words })).toHaveTextContent(String(sheets.length))
  })

  it('lists every sticker on hover, the used ones struck through', async () => {
    const user = userEvent.setup()
    draw(SHEETS)
    await user.hover(screen.getByRole('img', { name: /sticker sheets/ }))
    await waitFor(() => expect(screen.getAllByText('5/5 ({TK}{TK}{TK})').length).toBeGreaterThan(0))
    const used = screen.getAllByText('“Wicked” ({TK})')[0]!
    expect(used).toHaveAttribute('data-used', 'true')
    expect(screen.getAllByText('Flying ({TK}{TK})')[0]).toHaveAttribute('data-used', 'false')
  })

  it.each([[undefined], [[]]])('draws nothing without sheets', (sheets) => {
    const { container } = draw(sheets)
    expect(container).toBeEmptyDOMElement()
  })
})
