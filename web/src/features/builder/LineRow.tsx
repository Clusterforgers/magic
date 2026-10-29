import { HoverPicture } from '@/features/cards/CardPicture'
import { Trash2Icon, TriangleAlertIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SupportMark, supportEdge } from '@/features/decks/SupportMark'
import { FixMarkButton } from '@/features/fixmarks/FixMarkButton'
import { cn } from '@/lib/utils'
import type { FixMarksApi } from '@/net/fixMarks'
import type { Line } from './deckEdit'
import { Copies, MoveMenu } from './LineControls'
import type { LineActions } from './LineControls'
import { ManaCost } from './ManaCost'
import { isIssue, reasonLines } from './report'
import type { LineCheck } from './report'
import { ZONE_THE } from './zones'

export type { LineActions } from './LineControls'

interface LineRowProps {
  line: Line
  format: string
  /** False when the deck is at its card limit, so another copy cannot be added. */
  room: boolean
  /** What the deck check says of the card. */
  check: LineCheck
  /** The check's sentence when the card lies outside the commanders' colour identity, else null. */
  outside?: string | null
  actions: LineActions
  /** The fix-mark endpoints; with them a card that is not Verified gets the flag. */
  fixMarks?: FixMarksApi
}

/**
 * One line of the deck: the copies with minus and plus (the last copy removed takes the line out), the name (a button
 * that opens the card's preview), the cost, and a menu to move copies to another zone with Remove beside it. Under 640 px the row wraps to two lines. A card the
 * game cannot play by itself has an edge in its state's colour, the flag that marks it important to fix and, under the
 * name, its mark and the reasons the check gives.
 */
export function LineRow({ line, format, room, check, outside = null, actions, fixMarks }: LineRowProps) {
  const { zone, card } = line
  const id = card.oracle_id
  const issue = isIssue(check.support)
  const reasons = reasonLines(check.reasons)
  return (
    <li
      data-line={`${zone}:${id}`}
      data-support={check.support}
      data-outside={outside !== null || undefined}
      className={cn(
        'grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 gap-y-1 rounded-lg border-l-4 px-1 py-1 hover:bg-accent sm:grid-cols-[auto_minmax(0,1fr)_auto_auto]',
        issue ? supportEdge(check.support) : outside === null ? 'border-transparent' : 'border-destructive',
      )}
    >
      <Copies line={line} room={room} actions={actions} className="order-3 sm:order-none" />
      <HoverPicture
        oracleId={card.oracle_id}
        name={card.name}
        className="order-1 justify-self-start sm:order-none"
      >
        <button
          type="button"
          aria-haspopup="dialog"
          title={card.name}
          data-slot="card-name"
          onClick={(event) => actions.preview(card, event.currentTarget)}
          className="order-1 min-h-8 min-w-0 justify-self-start truncate rounded-sm text-left text-sm underline-offset-4 hover:underline pointer-coarse:min-h-11 sm:order-none"
        >
          {card.name}
        </button>
      </HoverPicture>
      <span className="order-2 justify-self-end sm:order-none">
        <ManaCost cost={card.mana_cost} />
      </span>
      <div className="order-4 flex items-center justify-self-end sm:order-none">
        {issue && fixMarks !== undefined ? (
          <FixMarkButton compact oracleId={id} name={card.name} api={fixMarks} emphasis="prominent" />
        ) : null}
        <MoveMenu line={line} format={format} actions={actions} />
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={`Remove ${card.name} from ${ZONE_THE[zone]}`}
          onClick={() => actions.remove(zone, id)}
        >
          <Trash2Icon aria-hidden="true" />
        </Button>
      </div>
      {issue ? (
        <div
          data-slot="line-check"
          className="order-5 col-span-full flex items-start gap-2 sm:order-none sm:col-span-3 sm:col-start-2"
        >
          <SupportMark support={check.support} />
          {reasons.length === 0 ? null : (
            <ul
              aria-label={`Why ${card.name} is not verified`}
              className="grid min-w-0 flex-1 gap-1 pt-1 text-xs"
            >
              {reasons.map((reason) => (
                <li key={reason.key} className="text-muted-foreground">
                  {reason.text}
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : null}
      {outside === null ? null : (
        <p
          data-slot="line-outside"
          className="order-5 col-span-full flex items-start gap-1 text-xs text-destructive sm:order-none sm:col-span-3 sm:col-start-2"
        >
          <TriangleAlertIcon aria-hidden="true" className="mt-1 size-4 shrink-0" />
          {outside}
        </p>
      )}
    </li>
  )
}
