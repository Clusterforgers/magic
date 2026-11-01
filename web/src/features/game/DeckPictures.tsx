import { LayersIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { knownStatus } from '@/features/cards/CardImage'
import { CardPicture } from '@/features/cards/CardPicture'
import { useImageChoice } from '@/features/cards/imageSetting'
import { createImageQueue } from '@/features/cards/imageQueue'
import type { ImageQueue, Ticket } from '@/features/cards/imageQueue'
import { cardImageUrl } from '@/features/cards/images'
import { cn } from '@/lib/utils'
import type { DeckEntry } from './deckEntries'

/** How many deck pictures are asked for at once, whatever the number of rows. */
export const PICTURES_AT_ONCE = 4

// One queue for every list of the page: 200 precons ask the image cache for a few pictures at a time.
const shared = createImageQueue(PICTURES_AT_ONCE)

/** The box of one picture, as wide as its skeleton and its tile, so nothing moves when the picture arrives. */
const BOX = 'aspect-[5/7] w-11 shrink-0 rounded-[4.75%/3.393%]'

/** Whether the element is in view now; true at once where the browser cannot tell. */
function useInView(ref: RefObject<Element | null>): boolean {
  const [seen, setSeen] = useState(() => typeof IntersectionObserver === 'undefined')
  useEffect(() => {
    const node = ref.current
    if (node === null || typeof IntersectionObserver === 'undefined') return
    const observer = new IntersectionObserver((entries) => {
      const last = entries[entries.length - 1]
      if (last !== undefined) setSeen(last.isIntersecting)
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [ref])
  return seen
}

/** The tile of a deck with no picture to ask for (or with pictures off): the set of a precon, else an icon. */
function Tile({ entry }: { entry: DeckEntry }) {
  return (
    <span
      aria-hidden="true"
      data-slot="deck-tile"
      className={cn(
        BOX,
        'grid place-items-center border bg-muted text-center text-label font-semibold tracking-wide text-muted-foreground',
      )}
    >
      {entry.set === null ? <LayersIcon className="size-5" /> : entry.set.toUpperCase()}
    </span>
  )
}

/**
 * One picture, asked for when its row is in view and a slot of the queue is free; until then, and while it loads, the
 * skeleton of the same size stands in. A picture the page already holds (or knows is missing) skips the queue.
 */
function LazyPicture({
  card,
  visible,
  queue,
}: {
  card: DeckEntry['pictures'][number]
  visible: boolean
  queue: ImageQueue
}) {
  const src = cardImageUrl({ kind: 'card', identity: { oracle_id: card.oracleId } }, 0, 'small')
  const settled = knownStatus(src) !== 'loading'
  const [granted, setGranted] = useState(false)
  const ticket = useRef<Ticket | null>(null)

  // Leaving the view before a slot is free gives the place up; a slot already held stays until the picture settles.
  useEffect(() => {
    if (!visible || granted || settled) return
    const mine = queue.request(() => setGranted(true))
    ticket.current = mine
    return () => mine.cancel()
  }, [visible, granted, settled, queue])
  useEffect(
    () => () => {
      ticket.current?.done()
    },
    [],
  )

  if (!granted && !settled) {
    return (
      <span aria-hidden="true" data-slot="deck-picture-wait" className={cn(BOX, 'animate-pulse bg-muted')} />
    )
  }
  return (
    <CardPicture
      oracleId={card.oracleId}
      name={card.name}
      size="small"
      alt=""
      quiet
      pulse
      // A picture that scrolled out of view after its slot was given would never load lazily and would hold the slot.
      eager
      className="w-11 shadow-e1"
      onSettle={() => ticket.current?.done()}
    />
  )
}

/**
 * The pictures of a row's deck, one or two overlapped: its commanders, else its first card. They are asked for when the
 * row scrolls into view, a few at a time (`PICTURES_AT_ONCE`), with the skeleton in place until each arrives.
 * `fallback` holds the cards of a deck whose view was read, for a list that knows none.
 */
export function DeckPictures({
  entry,
  fallback,
  queue = shared,
}: {
  entry: DeckEntry
  fallback: DeckEntry['pictures']
  queue?: ImageQueue
}) {
  const choice = useImageChoice()
  const group = useRef<HTMLSpanElement>(null)
  const visible = useInView(group)
  const cards = (entry.pictures.length > 0 ? entry.pictures : fallback).slice(0, 2)
  if (cards.length === 0 || choice === 'text') return <Tile entry={entry} />
  return (
    <span
      ref={group}
      aria-hidden="true"
      data-slot="deck-pictures"
      data-count={cards.length}
      className="flex shrink-0 -space-x-4"
    >
      {cards.map((card) => (
        <LazyPicture key={card.oracleId} card={card} visible={visible} queue={queue} />
      ))}
    </span>
  )
}
