import { TriangleAlertIcon } from 'lucide-react'
import { CardPicture, HoverPicture } from '@/features/cards/CardPicture'
import { SupportMark, supportRing } from '@/features/decks/SupportMark'
import { FixMarkButton } from '@/features/fixmarks/FixMarkButton'
import { cn } from '@/lib/utils'
import type { FixMarksApi } from '@/net/fixMarks'
import type { Line } from './deckEdit'
import { Copies, MoveMenu } from './LineControls'
import type { LineActions } from './LineControls'
import { isIssue } from './report'
import type { LineCheck } from './report'

interface CardTileProps {
  line: Line
  format: string
  /** False when the deck is at its card limit, so another copy cannot be added. */
  room: boolean
  /** What the deck check says of the card. */
  check: LineCheck
  /** The check's sentence when the card lies outside the commanders' colour identity, else null. */
  outside?: string | null
  actions: LineActions
  /** The fix-mark endpoints; with them a card that is not Verified gets the flag over its picture. */
  fixMarks?: FixMarksApi
}

/**
 * One line of the deck as a picture: the card large enough to read with a badge for its copies, its name (a button that
 * opens the preview, as in the list), minus and plus under it and a menu that moves or removes it. A card the game does
 * not run whole has a ring in its state's colour, its mark under the picture and the flag that marks it important to fix.
 */
export function CardTile({ line, format, room, check, outside = null, actions, fixMarks }: CardTileProps) {
  const { zone, card, count } = line
  const issue = isIssue(check.support)
  return (
    <li
      data-line={`${zone}:${card.oracle_id}`}
      data-support={check.support}
      data-outside={outside !== null || undefined}
      className="grid min-w-0 grid-cols-[minmax(0,1fr)] content-start gap-1"
    >
      <div className="relative">
        <HoverPicture oracleId={card.oracle_id} name={card.name} className="block">
          <button
            type="button"
            aria-haspopup="dialog"
            title={card.name}
            data-slot="card-name"
            onClick={(event) => actions.preview(card, event.currentTarget)}
            className="block w-full min-w-0 rounded-sm text-left outline-offset-2"
          >
            <span
              className={cn(
                'block rounded-[4.75%/3.393%] transition-transform duration-fast ease-out hover:-translate-y-0.5',
                issue
                  ? `ring-2 ${supportRing(check.support)}`
                  : outside !== null && 'ring-2 ring-destructive',
              )}
            >
              <CardPicture
                oracleId={card.oracle_id}
                name={card.name}
                size="normal"
                alt=""
                className="w-full"
              />
            </span>
            <span className="mt-1 block truncate text-xs font-medium">{card.name}</span>
          </button>
        </HoverPicture>
        {count > 1 ? (
          <span
            data-slot="count-badge"
            aria-hidden="true"
            className="pointer-events-none absolute -top-1.5 -right-1.5 grid min-w-6 place-items-center rounded-full border bg-popover px-1 text-xs font-semibold tabular-nums shadow-e1"
          >
            ×{count}
          </span>
        ) : null}
        {issue && fixMarks !== undefined ? (
          <FixMarkButton
            compact
            oracleId={card.oracle_id}
            name={card.name}
            api={fixMarks}
            className="absolute top-1 left-1 rounded-full bg-popover/90 shadow-e1"
          />
        ) : null}
      </div>
      {issue ? <SupportMark support={check.support} className="justify-self-start" /> : null}
      {outside === null ? null : (
        <p data-slot="line-outside" className="flex items-start gap-1 text-xs text-destructive">
          <TriangleAlertIcon aria-hidden="true" className="mt-1 size-4 shrink-0" />
          <span className="sr-only">{outside}</span>
          <span aria-hidden="true" title={outside}>
            Outside identity
          </span>
        </p>
      )}
      <div className="flex items-center justify-between gap-1">
        <Copies line={line} room={room} actions={actions} />
        <MoveMenu line={line} format={format} actions={actions} withRemove />
      </div>
    </li>
  )
}
