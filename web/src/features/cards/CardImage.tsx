import { useState } from 'react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

/** How a picture is doing: asked for, drawn, or missing (404, 502, a network error), when the text face stays. */
export type ImageStatus = 'loading' | 'loaded' | 'failed'

/** A missing picture is asked for again after this long, since a 502 may pass. */
export const RETRY_AFTER_MS = 5 * 60_000

// Shared by every picture on the page, so a remounted card neither flashes its skeleton nor asks for a missing one again.
const loaded = new Set<string>()
const failedAt = new Map<string, number>()
// Misses kept for the whole session, by key (`CardImage` `missKey`): a token that no picture stands for.
const missed = new Set<string>()

/** The status a picture starts in: what the page learned of it before; `key` is the miss key it is asked under. */
export function knownStatus(src: string, now: number = Date.now(), key: string = src): ImageStatus {
  if (loaded.has(src)) return 'loaded'
  if (missed.has(key)) return 'failed'
  const failed = failedAt.get(src)
  return failed !== undefined && now - failed < RETRY_AFTER_MS ? 'failed' : 'loading'
}

/** Forgets what the page learned of pictures (tests). */
export function resetImageMemory(): void {
  loaded.clear()
  failedAt.clear()
  missed.clear()
}

/** The corner of a printed card: 4.75% of its width, which is 3.39% of its height. */
export const CARD_CORNER = 'rounded-[4.75%/3.393%]'

interface CardImageProps {
  /** The picture's URL (`cardImageUrl`); null draws the skeleton alone. */
  src: string | null
  /** The card's name; "" for a picture that repeats a name said beside it. */
  alt: string
  /** The picture asked for once `src` is missing (`cardImageFallbackUrl`); the text face stays when both are. */
  fallback?: string | null
  /**
   * A miss is remembered for the session under this key instead of for `RETRY_AFTER_MS` under the URL, so every URL
   * that shares it (a token at other power and toughness, which the server finds by name) is not asked for again.
   */
  missKey?: string | null
  /**
   * The text face under the picture: shown while it loads, when it is missing and when pictures are off. A function
   * gets the status, to draw a missing picture's face differently from the one that waits.
   */
  children?: ReactNode | ((status: ImageStatus) => ReactNode)
  /** State drawn over a loaded picture, never into it. */
  overlay?: ReactNode
  /** The box; a 5:7 card as wide as its parent when left out. */
  className?: string
  /** The picture lies on its side in a landscape box, as a tapped permanent does. */
  turned?: boolean
  /** Ask for it at once, as the preview does; lazy otherwise. */
  eager?: boolean
}

/**
 * A card's picture over its text face. The picture is a plain `<img>`: lazy, decoded off the main thread, never
 * cropped, recoloured or marked; the state over it is a separate layer. Until it loads, and whenever it is missing, the
 * text face shows and no broken-image icon ever does.
 */
export function CardImage({
  src: asked,
  fallback = null,
  missKey = null,
  alt,
  children,
  overlay,
  className,
  turned = false,
  eager = false,
}: CardImageProps) {
  const [seen, setSeen] = useState<{ src: string; status: ImageStatus } | null>(null)
  const keyOf = (url: string): string => (url === asked && missKey !== null ? missKey : url)
  const statusOf = (url: string): ImageStatus =>
    seen?.src === url ? seen.status : knownStatus(url, Date.now(), keyOf(url))
  const src =
    asked !== null && fallback !== null && fallback !== asked && statusOf(asked) === 'failed'
      ? fallback
      : asked
  const status: ImageStatus | null = src === null ? null : statusOf(src)
  const settle = (next: ImageStatus) => {
    if (src === null) return
    if (next === 'loaded') loaded.add(src)
    else if (keyOf(src) !== src) missed.add(keyOf(src))
    else failedAt.set(src, Date.now())
    setSeen({ src, status: next })
  }
  return (
    <span
      data-slot="card-image"
      data-status={status ?? 'none'}
      className={cn('relative block', className ?? 'aspect-[5/7] w-full')}
    >
      {status === 'loaded' ? null : typeof children === 'function' ? children(status ?? 'failed') : children}
      {src === null || status === 'failed' ? null : (
        <span
          aria-hidden={status === 'loaded' ? undefined : true}
          className={cn(
            'absolute',
            turned
              ? 'top-1/2 left-1/2 h-[140%] w-[71.43%] -translate-x-1/2 -translate-y-1/2 rotate-90'
              : 'inset-0',
          )}
        >
          <img
            ref={(img) => {
              // A picture the browser already holds (a reload, a card that came back) is drawn in the first frame.
              if (img !== null && status === 'loading' && img.complete && img.naturalWidth > 0)
                settle('loaded')
            }}
            src={src}
            alt={alt}
            loading={eager ? 'eager' : 'lazy'}
            decoding="async"
            draggable={false}
            onLoad={() => settle('loaded')}
            onError={() => settle('failed')}
            className={cn('block size-full shadow-sm', CARD_CORNER, status !== 'loaded' && 'opacity-0')}
          />
        </span>
      )}
      {status === 'loaded' ? overlay : null}
    </span>
  )
}

/** The back of a card nobody at this seat may see, or one lying face down: our own neutral design. */
export function CardBack({ className, label }: { className?: string; label?: string }) {
  return (
    <span
      data-slot="card-back"
      role={label === undefined ? undefined : 'img'}
      aria-label={label}
      className={cn(
        'relative grid place-items-center overflow-hidden border-[3px] border-card-back-edge bg-card-back',
        CARD_CORNER,
        className ?? 'aspect-[5/7] w-full',
      )}
    >
      <svg viewBox="0 0 50 70" aria-hidden="true" className="size-full text-card-back-ink">
        <rect x="6" y="6" width="38" height="58" rx="3" fill="none" stroke="currentColor" strokeWidth="1.2" />
        <path d="M25 18 L37 35 L25 52 L13 35 Z" fill="none" stroke="currentColor" strokeWidth="1.6" />
        <circle cx="25" cy="35" r="4" fill="currentColor" />
      </svg>
    </span>
  )
}
