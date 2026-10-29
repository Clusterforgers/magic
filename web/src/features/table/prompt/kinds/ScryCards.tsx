import { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { CardsOptions, ObjectId } from '@/protocol/generated'
import { distinctCards } from '../cardsText'
import { askerWords, numberKey } from '../choiceText'
import { uniqueNames } from '../combatText'
import { awayPile, lookKeys, lookPreview, lookProgress, lookTitle, splitLooked, TOP_PILE } from '../lookText'
import type { Away } from '../lookText'
import { usePrompt } from '../PromptContext'
import { PromptCard } from '../PromptCard'
import { PromptLayout } from '../PromptLayout'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/** Up to this many cards are drawn whole; more use the compact face, so the piles sit side by side in a narrow prompt. */
const NORMAL_UP_TO = 3

interface PileProps {
  title: string
  ids: readonly ObjectId[]
  slot: 'top' | 'away'
  children: ReactNode
}

/** One pile of the split: its heading with the count, and the cards (or a note that it is empty). */
function Pile({ title, ids, slot, children }: PileProps) {
  return (
    <div
      role="group"
      aria-label={`${title}: ${ids.length}`}
      data-slot={`look-${slot}`}
      className={cn(
        // Each pile is as wide as its cards and the empty one as its note, so four cards and an empty pile share one row.
        'flex min-w-0 max-w-full grow flex-col gap-1 rounded-md border border-border p-1.5',
        slot === 'away' && 'bg-muted/40',
      )}
    >
      <p className="text-xs font-medium text-muted-foreground" aria-hidden="true">
        {title} <span className="tabular-nums">({ids.length})</span>
      </p>
      <div className="flex flex-wrap content-start gap-1.5">
        {ids.length === 0 ? (
          <p data-slot="pile-empty" className="p-2 text-xs text-muted-foreground">
            No cards
          </p>
        ) : (
          children
        )}
      </div>
    </div>
  )
}

/**
 * The split of a scry or a surveil: the cards looked at, as pictures numbered from the top of the library, in two
 * piles, the cards that stay on top and the cards that go away (the bottom of the library, or the graveyard). A click,
 * Enter, Space or the card's number moves a card to the other pile; a line under the buttons shows the result.
 * Confirm answers the cards that go away, in the order looked at, the `ChooseCards` answer the engine expects; the
 * order of each pile is asked next, as an `OrderCards`. The server's answer at the deadline is none of them, so
 * everything stays on top.
 */
export function ScryCards({ options, kind }: { options: CardsOptions; kind: Away }) {
  const { answer } = usePrompt()
  const looked = useMemo(() => distinctCards(options.from), [options.from])
  const known = useObjects(useMemo(() => [options.source, ...looked], [options.source, looked]))
  const objects = useMemo(() => looked.map((_, place) => known[place + 1]), [looked, known])
  const labels = useMemo(
    () => uniqueNames(looked.map((id, place) => ({ id, name: objects[place]?.name ?? 'A card' }))),
    [looked, objects],
  )
  const [away, setAway] = useState<ReadonlySet<ObjectId>>(() => new Set())
  const first = useRef<HTMLButtonElement>(null)
  useDefaultFocus(first)
  const piles = useRef<HTMLDivElement>(null)
  // A card that changes piles is a new element in the page, which drops focus; a press on the card brings it back.
  const refocus = useRef<ObjectId | null>(null)
  useEffect(() => {
    const id = refocus.current
    refocus.current = null
    if (id === null) return
    piles.current?.querySelector<HTMLButtonElement>(`button[data-candidate="${id}"]`)?.focus()
  }, [away])

  const press = (id: ObjectId, keep = false): void => {
    refocus.current = keep ? null : id
    setAway((now) => {
      const next = new Set(now)
      if (!next.delete(id)) next.add(id)
      return next
    })
  }
  usePromptKeys(
    Object.fromEntries(
      looked.slice(0, 10).map((id, place) => [numberKey(place) ?? '', () => press(id, true)]),
    ),
  )

  const split = splitLooked(looked, away)
  const nameOf = (id: ObjectId): string => labels.get(id) ?? 'A card'
  const asker = askerWords(known[0]?.name)
  const words = options.text ?? ''
  const moved = away.size > 0
  const text = (
    <>
      {moved
        ? lookProgress(asker, split.top.length, split.away.length, kind)
        : `${asker}: ${words === '' ? lookTitle(kind, looked.length) : words}`}
      <span className="sr-only"> {lookKeys(looked.length)}</span>
    </>
  )
  const size = looked.length <= NORMAL_UP_TO ? 'normal' : 'compact'
  const card = (id: ObjectId) => {
    const place = looked.indexOf(id)
    return (
      <PromptCard
        key={id}
        ref={place === 0 ? first : undefined}
        id={id}
        label={`${nameOf(id)}, ${away.has(id) ? awayPile(kind).toLowerCase() : 'on top'}, card ${place + 1} of ${looked.length}`}
        size={size}
        chosen={away.has(id)}
        closed={false}
        tabbable
        hotkey={numberKey(place)}
        onPress={press}
      />
    )
  }
  return (
    <PromptLayout text={text}>
      <Button className="h-9 px-4" onClick={() => answer({ ChooseCards: split.away })}>
        Confirm
      </Button>
      <p
        data-slot="look-preview"
        aria-hidden="true"
        className="line-clamp-2 min-w-0 basis-full text-xs text-muted-foreground"
      >
        {lookPreview(split.top.map(nameOf), split.away.map(nameOf), kind)}
      </p>
      <div
        ref={piles}
        data-slot="look-piles"
        className="flex basis-full flex-wrap gap-2 overflow-y-auto max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_10.5rem))]"
      >
        <Pile title={TOP_PILE} ids={split.top} slot="top">
          {split.top.map(card)}
        </Pile>
        <Pile title={awayPile(kind)} ids={split.away} slot="away">
          {split.away.map(card)}
        </Pile>
      </div>
    </PromptLayout>
  )
}
