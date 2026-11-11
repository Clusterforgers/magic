import { useState } from 'react'
import type { ReactElement, PointerEvent as ReactPointerEvent } from 'react'
import { SupportMark } from '@/features/decks/SupportMark'
import { cardWords } from '@/lib/lineMarks'
import { cn } from '@/lib/utils'
import type { Support } from '@/protocol/generated'
import { CardImage } from './CardImage'
import { hoverFit, HOVER_PADDING_PX } from './hoverFit'
import type { HoverFit } from './hoverFit'
import { useImageChoice } from './imageSetting'
import { cardImageUrl } from './images'
import type { ImageSize } from './images'

interface CardPictureProps {
  oracleId: string
  /** The card's (or face's) name: the picture's accessible name and the skeleton's text. */
  name: string
  face?: number
  size?: ImageSize
  /** The box; its 5:7 shape is reserved before the picture loads, so nothing moves. */
  className?: string
  /** "" when the name is said beside the picture. */
  alt?: string
  /** Leaves the name off the skeleton, for a thumbnail too small to read it. */
  quiet?: boolean
  /** Pulses the skeleton while the picture loads. */
  pulse?: boolean
  /** Asks for the picture at once instead of when it nears the view. */
  eager?: boolean
  /** Told once when the picture has loaded or is missing. */
  onSettle?(status: 'loaded' | 'failed'): void
}

/** A card's picture by oracle id outside the table, over a plain skeleton that names it; none with Text only. */
export function CardPicture({
  oracleId,
  name,
  face = 0,
  size = 'normal',
  className,
  alt,
  quiet = false,
  pulse = false,
  eager = false,
  onSettle,
}: CardPictureProps) {
  const choice = useImageChoice()
  if (choice === 'text') return null
  return (
    <CardImage
      src={cardImageUrl({ kind: 'card', identity: { oracle_id: oracleId } }, face, size)}
      alt={alt ?? name}
      onSettle={onSettle}
      eager={eager}
      className={cn('block aspect-[5/7] shrink-0', className ?? 'w-40')}
    >
      {(status) => (
        <span
          aria-hidden="true"
          className={cn(
            'grid size-full place-items-center rounded-[4.75%/3.393%] border bg-muted p-2 text-center text-xs text-muted-foreground',
            pulse && status === 'loading' && 'animate-pulse',
          )}
        >
          {quiet ? null : name}
        </span>
      )}
    </CardImage>
  )
}

/** What the hover preview says under the picture, beside the name. */
export interface HoverInfo {
  /** The type line of the whole card. */
  typeLine?: string
  /** The copies in the deck; said as "×N" from two. */
  count?: number
  /** How far the game plays the card: its mark and, when it is not Verified, the words for it. */
  support?: Support
  /** Why the card is not Verified, one line each. */
  reasons?: readonly string[]
}

/**
 * Wraps a card's name or tile so hovering it shows the card large, as the table's preview does: a panel docked at the
 * side of the window away from the element (`hoverFit`), with the picture up to 480 px wide and, under it, the name,
 * the type line and how far the game plays the card. For a pointer only (the element's own click serves touch and the
 * keyboard). The panel is fixed to the window, so a scrolling list never clips it, and it takes no pointer events.
 */
export function HoverPicture({
  oracleId,
  name,
  info,
  children,
  className,
}: {
  oracleId: string
  name: string
  info?: HoverInfo
  children: ReactElement
  /** Classes of the wrapper, which takes the name's place in its layout. */
  className?: string
}) {
  const [at, setAt] = useState<HoverFit | null>(null)
  const show = (event: ReactPointerEvent<HTMLElement>) => {
    if (event.pointerType !== 'mouse') return
    const box = event.currentTarget.getBoundingClientRect()
    setAt(hoverFit(box, { width: window.innerWidth, height: window.innerHeight }))
  }
  const reasons = [...new Set(info?.reasons ?? [])]
  return (
    <span className={cn('flex min-w-0', className)} onPointerEnter={show} onPointerLeave={() => setAt(null)}>
      {children}
      {at === null ? null : (
        <span
          data-slot="hover-picture"
          data-dock={at.dock}
          className="pointer-events-none fixed z-50 grid content-start gap-2 overflow-hidden rounded-xl bg-popover p-3 text-left text-sm text-popover-foreground shadow-lg ring-1 ring-foreground/15"
          style={{ left: at.left, top: at.top, width: at.width + HOVER_PADDING_PX, maxHeight: at.maxHeight }}
        >
          <CardPicture oracleId={oracleId} name={name} alt="" className="w-full" />
          <span className="grid gap-1">
            <span className="font-semibold">
              {name}
              {info?.count !== undefined && info.count > 1 ? (
                <span className="font-normal text-muted-foreground"> ×{info.count}</span>
              ) : null}
            </span>
            {info?.typeLine === undefined ? null : (
              <span className="text-muted-foreground">{info.typeLine}</span>
            )}
            {info?.support === undefined ? null : (
              <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                <SupportMark support={info.support} />
                {info.support === 'verified' ? null : (
                  <span className="text-xs font-medium">{cardWords(info.support)}</span>
                )}
              </span>
            )}
            {reasons.length === 0 ? null : (
              <span className="grid gap-0.5 text-xs text-muted-foreground">
                {reasons.map((reason) => (
                  <span key={reason} className="[overflow-wrap:anywhere]">
                    {reason}
                  </span>
                ))}
              </span>
            )}
          </span>
        </span>
      )}
    </span>
  )
}
