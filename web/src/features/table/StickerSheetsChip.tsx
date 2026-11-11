/** The sticker slot of a seat panel: how many sheets the seat plays with, and every sticker on them on hover or focus. */
import { Sticker } from 'lucide-react'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import type { PlayerView, StickerSheetView } from '@/protocol/generated'
import { CHIP } from './PodParts'

/** The words a screen reader hears for the chip: the sheets and how many stickers are left on them. */
export function stickerSheetsWords(sheets: StickerSheetView[]): string {
  const left = sheets.reduce((all, sheet) => all + sheet.stickers.filter((sticker) => !sticker.used).length, 0)
  const noun = sheets.length === 1 ? 'sticker sheet' : 'sticker sheets'
  return `${sheets.length} ${noun}, ${left} ${left === 1 ? 'sticker' : 'stickers'} left: ${sheets.map((sheet) => sheet.name).join(', ')}`
}

/** One sheet: its name and its stickers, a used one struck through. */
function Sheet({ sheet }: { sheet: StickerSheetView }) {
  return (
    <div data-slot="sticker-sheet" className="grid gap-0.5">
      <span className="font-medium">{sheet.name}</span>
      <ul className="grid gap-0.5">
        {sheet.stickers.map((sticker, index) => (
          <li key={index} data-used={sticker.used} className={cn(sticker.used && 'line-through opacity-60')}>
            {sticker.text}
          </li>
        ))}
      </ul>
    </div>
  )
}

/** A seat's sticker sheets (CR 123); nothing while it plays with none. */
export function StickerSheetsChip({ player }: { player: PlayerView }) {
  const sheets = player.sticker_sheets ?? []
  if (sheets.length === 0) return null
  const chip = (
    <span
      role="img"
      aria-label={stickerSheetsWords(sheets)}
      data-slot="sticker-sheets"
      tabIndex={0}
      className={cn(CHIP, 'bg-muted px-1.5 text-muted-foreground')}
    >
      <Sticker aria-hidden="true" className="size-4 shrink-0" />
      <span aria-hidden="true">{sheets.length}</span>
    </span>
  )
  return (
    <Tooltip>
      <TooltipTrigger render={chip} />
      <TooltipContent side="bottom">
        <div className="grid gap-2">
          {sheets.map((sheet, index) => (
            <Sheet key={index} sheet={sheet} />
          ))}
        </div>
      </TooltipContent>
    </Tooltip>
  )
}
