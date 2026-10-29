import { useState } from 'react'
import type { ReactElement, PointerEvent as ReactPointerEvent } from 'react'
import { cn } from '@/lib/utils'
import { CardImage } from './CardImage'
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
}: CardPictureProps) {
  const choice = useImageChoice()
  if (choice === 'text') return null
  return (
    <CardImage
      src={cardImageUrl({ kind: 'card', identity: { oracle_id: oracleId } }, face, size)}
      alt={alt ?? name}
      className={cn('block aspect-[5/7] shrink-0', className ?? 'w-40')}
    >
      <span
        aria-hidden="true"
        className="grid size-full place-items-center rounded-[4.75%/3.393%] border bg-muted p-2 text-center text-xs text-muted-foreground"
      >
        {quiet ? null : name}
      </span>
    </CardImage>
  )
}

/** Where the hover picture stands: beside the element, kept inside the window. */
function placeBeside(rect: DOMRect): { left: number; top: number } {
  const width = 244
  const height = 342
  const right = rect.right + 12
  const left = right + width <= window.innerWidth ? right : Math.max(8, rect.left - width - 12)
  const top = Math.min(Math.max(8, rect.top - height / 3), Math.max(8, window.innerHeight - height - 8))
  return { left, top }
}

/**
 * Wraps a card's name so hovering it shows the card's picture beside it, for a pointer only (the name's own preview
 * dialog serves the keyboard). The picture is fixed to the window, so a scrolling list never clips it.
 */
export function HoverPicture({
  oracleId,
  name,
  children,
  className,
}: {
  oracleId: string
  name: string
  children: ReactElement
  /** Classes of the wrapper, which takes the name's place in its layout. */
  className?: string
}) {
  const [at, setAt] = useState<{ left: number; top: number } | null>(null)
  const show = (event: ReactPointerEvent<HTMLElement>) => {
    if (event.pointerType === 'mouse') setAt(placeBeside(event.currentTarget.getBoundingClientRect()))
  }
  return (
    <span className={cn('flex min-w-0', className)} onPointerEnter={show} onPointerLeave={() => setAt(null)}>
      {children}
      {at === null ? null : (
        <span
          data-slot="hover-picture"
          className="pointer-events-none fixed z-50 w-61 drop-shadow-lg"
          style={{ left: at.left, top: at.top }}
        >
          <CardPicture oracleId={oracleId} name={name} alt="" className="w-61" />
        </span>
      )}
    </span>
  )
}
