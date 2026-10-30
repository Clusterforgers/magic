/**
 * The state of a permanent over its picture, in small marks at the corners: the number of damage, the counters and what
 * is attached at the foot (where a card prints its artist line), the identity and combat marks at the head (over the
 * mana cost, which a permanent no longer needs), a sick permanent as a small icon and a slight dim, and the current
 * power and toughness or loyalty on the stat box. Nothing is drawn over the art or the rules text beyond a mark's size.
 */
import { Hourglass, Link2, Unlink } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import { counterChips, loyaltyOf, statusChips } from './cardModel'
import type { ChipTone, CombatRole, CounterChip, Links } from './cardModel'
import { CARD_CORNER } from '@/features/cards/CardImage'
import { SIZE } from './cardSize'
import { roomFor } from './attachMarks'
import { GrantMarks } from './GrantMarks'
import { isUnattached } from '@/play/hosts'
import type { CardSize } from './cardSize'
import { ByHandMark } from './manual/ByHand'
import { statBadge } from './overlay'
import { usePrinted } from './printed'
import type { CardNote } from './prompt/marks'

// The foot stack grows up from the bottom left corner (the artist line), the head stack down from the top right one
// (the mana cost). A picture that lies on its side has turned a quarter clockwise, so the corners moved with it; its
// stat box is at the bottom left.
const CORNER = {
  upright: {
    foot: 'bottom-[2.5%] left-[3%] max-w-[62%] flex-col-reverse items-start',
    head: 'top-[1.5%] right-[3%] max-w-[70%] flex-col items-end',
  },
  turned: {
    foot: 'top-[3%] left-[2.5%] max-w-[62%] flex-col items-start',
    head: 'right-[2.5%] bottom-[3%] max-w-[70%] flex-col-reverse items-end',
  },
}
// The share of a face's width that the foot stack may take (its `max-w-[62%]`).
const FOOT_SHARE = 0.62
// A damage, counter or link mark is a line of the page's 24 px and the 2 px gap under it high.
const OTHER_MARK_PX = 26
// The power/toughness or loyalty box at the bottom right, which lies at the bottom left once turned.
const STAT_BOX = { upright: 'right-[3%] bottom-[2.5%]', turned: 'bottom-[3%] left-[2.5%]' }

const MARK =
  'flex h-fit max-w-full items-center gap-0.5 rounded-sm px-1 leading-4 font-semibold whitespace-nowrap shadow-sm'

const TONE: Record<ChipTone, string> = {
  plain: 'bg-foreground text-background',
  danger: 'bg-destructive text-background',
}

const COUNTER_TONE: Record<CounterChip['tone'], string> = {
  up: 'bg-support-verified text-background',
  down: 'bg-destructive text-background',
  plain: 'bg-foreground text-background',
}

/** What the overlay reads. */
export interface OverlayProps {
  object: ObjectView
  role: CombatRole | null
  size: CardSize
  turned: boolean
  note?: CardNote | undefined
  links?: Links | undefined
  /** The drawn width of the face in px: it decides how many marks the foot stack has room for. */
  width: number
}

/** The marks over a picture, or over the back of a face-down card. */
export function Overlay({ object, role, size, turned, note, links, width }: OverlayProps) {
  const text = SIZE[size].text
  const printed = usePrinted(object)
  const badge = statBadge(object, printed, loyaltyOf(object))
  const manual = object.manual ?? []
  const chips = statusChips(object, role)
  const damage = chips.find((chip) => chip.kind === 'damage')
  const sick = chips.some((chip) => chip.kind === 'sick')
  const head = chips.filter((chip) => chip.kind === 'role' || chip.kind === 'identity')
  const counters = counterChips(object)
  const host = links?.host
  const attaches = links?.attaches ?? []
  const corner = CORNER[turned ? 'turned' : 'upright']
  const unattached = isUnattached(object)
  const hasFoot =
    damage !== undefined || counters.length > 0 || host !== undefined || attaches.length > 0 || unattached
  // The marks of the attachments get the lines of the foot stack that the damage, counters and links leave.
  const others =
    (damage === undefined ? 0 : 1) + counters.length + (host === undefined ? 0 : 1) + (unattached ? 1 : 0)
  const room = roomFor(turned ? (width * 5) / 7 : (width * 7) / 5, others * OTHER_MARK_PX)
  const hasHead = head.length > 0 || sick || manual.length > 0 || note?.label !== undefined
  return (
    <span data-slot="card-overlay" className="pointer-events-none absolute inset-0">
      {sick ? (
        <span
          aria-hidden="true"
          data-slot="card-dim"
          className={cn('absolute inset-0 bg-background/25', CARD_CORNER)}
        />
      ) : null}
      {hasFoot ? (
        <span
          data-slot="card-marks-foot"
          className={cn('absolute flex max-h-[40%] gap-0.5 overflow-hidden', corner.foot)}
        >
          {damage === undefined ? null : (
            <span
              data-slot="card-status"
              data-tone={damage.tone}
              className={cn(MARK, TONE[damage.tone], text)}
            >
              {damage.label}
            </span>
          )}
          {counters.map((counter) => (
            <span
              key={counter.key}
              data-slot="card-counter"
              data-tone={counter.tone}
              className={cn(MARK, COUNTER_TONE[counter.tone], text)}
            >
              <span className="truncate">
                {counter.count === 1 ? counter.symbol : `${counter.symbol} ×${counter.count}`}
              </span>
            </span>
          ))}
          {host === undefined ? null : (
            <span
              data-slot="card-attached-to"
              className={cn(MARK, 'border border-foreground bg-background text-foreground', text)}
            >
              <Link2 aria-hidden="true" className="size-3 shrink-0" />
              {size === 'compact' ? null : <span className="truncate">{host}</span>}
            </span>
          )}
          <GrantMarks
            attaches={attaches}
            compact={size === 'compact'}
            room={room}
            span={width * FOOT_SHARE}
            up={!turned}
            text={text}
          />
          {unattached ? (
            <span
              data-slot="card-unattached"
              className={cn(
                MARK,
                'border border-dashed border-foreground bg-background text-foreground',
                text,
              )}
            >
              <Unlink aria-hidden="true" className="size-3 shrink-0" />
              <span className={cn('truncate', size === 'compact' && 'sr-only')}>Unattached</span>
            </span>
          ) : null}
        </span>
      ) : null}
      {hasHead ? (
        <span
          data-slot="card-marks-head"
          className={cn('absolute flex max-h-[34%] gap-0.5 overflow-hidden', corner.head)}
        >
          <ByHandMark object={object} spot="corner" compact={size === 'compact'} text={text} />
          {sick ? (
            <span
              data-slot="card-sick"
              className="grid size-4 place-items-center rounded-full bg-foreground text-background shadow-sm"
            >
              <Hourglass aria-hidden="true" className="size-2.5" />
              <span className="sr-only">Summoning sick</span>
            </span>
          ) : null}
          {head.map(({ label, tone }) => (
            <span key={label} data-slot="card-status" data-tone={tone} className={cn(MARK, TONE[tone], text)}>
              {label}
            </span>
          ))}
          {note?.label === undefined ? null : (
            <span data-slot="card-note" className={cn(MARK, 'bg-primary text-primary-foreground', text)}>
              <span className="truncate">{note.label}</span>
            </span>
          )}
        </span>
      ) : null}
      {badge === null ? null : (
        <span
          data-slot={badge.kind === 'loyalty' ? 'card-loyalty' : 'card-pt'}
          data-modified={badge.modified ?? undefined}
          className={cn(
            'absolute rounded-md border px-1.5 leading-snug font-bold tabular-nums shadow-sm',
            SIZE[size].name,
            turned ? STAT_BOX.turned : STAT_BOX.upright,
            badge.modified === 'up'
              ? 'border-background bg-support-verified text-background'
              : badge.modified === 'down'
                ? 'border-background bg-destructive text-background'
                : 'border-background bg-foreground text-background',
          )}
        >
          {badge.text}
        </span>
      )}
    </span>
  )
}
