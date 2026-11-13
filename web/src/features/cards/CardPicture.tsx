import { useCallback, useEffect, useId, useState } from 'react'
import type {
  FocusEvent as ReactFocusEvent,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  ReactElement,
} from 'react'
import { createPortal } from 'react-dom'
import { XIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SupportMark } from '@/features/decks/SupportMark'
import { cardWords } from '@/lib/lineMarks'
import { cn } from '@/lib/utils'
import type { Support } from '@/protocol/generated'
import { CardImage } from './CardImage'
import { hoverFit, HOVER_PADDING_PX } from './hoverFit'
import { hoverPreview, useHoverTarget } from './hoverPreview'
import { useImageChoice } from './imageSetting'
import { cardImageFallbackUrl, cardImageUrl, identityOf } from './images'
import type { ImageSize } from './images'

interface CardPictureProps {
  oracleId: string
  /** The printing whose art is drawn; the card's default art when left out or null. */
  printingId?: string | null
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
  printingId = null,
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
      src={cardImageUrl({ kind: 'card', identity: identityOf(oracleId, printingId) }, face, size)}
      fallback={
        printingId === null
          ? null
          : cardImageFallbackUrl({ kind: 'card', identity: identityOf(oracleId, printingId) }, face, size)
      }
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
 * the type line and how far the game plays the card. Hover is for a mouse only. Every wrapper shares one preview
 * (`hoverPreview`): with `pin`, a click (or Enter and Space, focus showing it) pins the panel until its Close button,
 * Escape or a second click on the same card, and while a card is pinned other hovers show nothing. Without `pin` the
 * element's own click acts (it opens a dialog) and pins nothing. The panel is drawn into the page's body, or into the
 * dialog around the element, so no clipping or contained ancestor moves or hides it.
 */
export function HoverPicture({
  oracleId,
  printingId = null,
  name,
  info,
  children,
  className,
  pin = false,
}: {
  oracleId: string
  /** The printing whose art the preview draws; the default art when left out or null. */
  printingId?: string | null
  name: string
  info?: HoverInfo
  children: ReactElement
  /** Classes of the wrapper, which takes the name's place in its layout. */
  className?: string
  /** A click pins the preview; for a picture or name that has no click of its own. */
  pin?: boolean
}) {
  const key = useId()
  const target = useHoverTarget()
  const choice = useImageChoice()
  // The dialog around the element, if any: a modal dialog takes a press outside it as a request to close.
  const [host, setHost] = useState<Element | null>(null)
  const wrapper = useCallback((element: HTMLSpanElement | null) => {
    if (element !== null) setHost(element.closest('[role="dialog"]') ?? document.body)
  }, [])
  const shown = target?.key === key ? target : null
  const pinned = shown?.pinned === true
  // A wrapper that leaves the page takes its preview with it, pinned or not.
  useEffect(
    () => () => {
      if (hoverPreview.get()?.key === key) hoverPreview.dismiss()
    },
    [key],
  )
  useEffect(() => {
    if (!pinned) return
    const close = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      hoverPreview.dismiss()
    }
    document.addEventListener('keydown', close)
    return () => document.removeEventListener('keydown', close)
  }, [pinned])
  // With Text only a bare picture draws nothing, so there is nothing to hover or press.
  if (pin && choice === 'text' && children.type === CardPicture) return null
  const fitOf = (element: HTMLElement) =>
    hoverFit(element.getBoundingClientRect(), { width: window.innerWidth, height: window.innerHeight })
  const press = (element: HTMLElement) => hoverPreview.pin(key, fitOf(element))
  const reasons = [...new Set(info?.reasons ?? [])]
  return (
    <span
      ref={wrapper}
      className={cn('flex min-w-0', pin && 'cursor-zoom-in', className)}
      data-slot="hover-target"
      data-pinned={pinned || undefined}
      onPointerEnter={(event) => {
        if (event.pointerType === 'mouse') hoverPreview.show(key, fitOf(event.currentTarget))
      }}
      onPointerLeave={() => hoverPreview.hide(key)}
      {...(pin
        ? {
            role: 'button',
            tabIndex: 0,
            'aria-label': `Show ${name} large`,
            'aria-pressed': pinned,
            onClick: (event: ReactMouseEvent<HTMLElement>) => press(event.currentTarget),
            onKeyDown: (event: ReactKeyboardEvent<HTMLElement>) => {
              if (event.key !== 'Enter' && event.key !== ' ') return
              event.preventDefault()
              press(event.currentTarget)
            },
            onFocus: (event: ReactFocusEvent<HTMLElement>) =>
              hoverPreview.show(key, fitOf(event.currentTarget)),
            onBlur: () => hoverPreview.hide(key),
          }
        : {})}
    >
      {children}
      {shown === null || host === null
        ? null
        : createPortal(
            <span
              role={pinned ? 'group' : undefined}
              aria-label={pinned ? 'Card preview' : undefined}
              data-slot="hover-picture"
              data-dock={shown.at.dock}
              data-pinned={pinned || undefined}
              className={cn(
                'fixed z-[70] grid content-start gap-2 overflow-y-auto rounded-xl bg-popover p-3 text-left text-sm text-popover-foreground shadow-lg ring-1 ring-foreground/15',
                pinned ? null : 'pointer-events-none',
              )}
              // The panel sits inside the wrapper in React's tree: its presses must not pin or unpin the card again.
              onClick={(event) => event.stopPropagation()}
              onKeyDown={(event) => event.stopPropagation()}
              style={{
                left: shown.at.left,
                top: shown.at.top,
                width: shown.at.width + HOVER_PADDING_PX,
                maxHeight: shown.at.maxHeight,
              }}
            >
              <CardPicture
                oracleId={oracleId}
                printingId={printingId}
                name={name}
                alt=""
                className="w-full"
              />
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
              {pinned ? (
                <Button
                  variant="outline"
                  size="sm"
                  className="justify-self-start"
                  onClick={() => hoverPreview.dismiss()}
                >
                  <XIcon aria-hidden="true" />
                  Close preview
                </Button>
              ) : null}
            </span>,
            host,
          )}
    </span>
  )
}
