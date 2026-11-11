import { BookOpenIcon, CheckIcon, HandIcon, TriangleAlertIcon, XIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { Ref } from 'react'
import { Kbd } from '@/components/Kbd'
import { cn } from '@/lib/utils'
import type { ReadTarget } from '@/lib/toRead'
import { allInDecks, rankText } from './model'
import { REVIEW_KEYS, rowState } from './review'
import type { Outcome, ReviewItem, RowTone } from './review'

const TONE_ICON: Readonly<Record<RowTone, { icon: LucideIcon; className: string }>> = {
  to_read: { icon: BookOpenIcon, className: 'text-support-read' },
  right: { icon: CheckIcon, className: 'text-support-verified' },
  wrong: { icon: XIcon, className: 'text-destructive' },
  by_hand: { icon: HandIcon, className: 'text-support-manual' },
  checked: { icon: CheckIcon, className: 'text-support-verified' },
}

interface ReviewListProps {
  items: readonly ReviewItem[]
  done: ReadonlyMap<string, Outcome>
  /** The item on show. */
  shown: string | null
  onPick: (id: string) => void
  target: ReadTarget | null
  listRef?: Ref<HTMLDivElement>
}

function Row({
  item,
  outcome,
  current,
  target,
  onPick,
}: {
  item: ReviewItem
  outcome: Outcome | undefined
  current: boolean
  target: ReadTarget | null
  onPick: () => void
}) {
  const state = rowState(item, outcome)
  const { icon: Icon, className } = TONE_ICON[state.tone]
  const rank = item.kind === 'read' ? rankText(item.card.rank) : null
  const differs = item.kind === 'read' ? item.card.differs : 0
  return (
    <li>
      <button
        type="button"
        aria-current={current ? 'true' : undefined}
        data-item={item.id}
        data-tone={state.tone}
        onClick={onPick}
        className={cn(
          'grid w-full grid-cols-1 gap-0.5 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent',
          current ? 'bg-accent font-medium' : null,
          outcome === undefined ? null : 'text-muted-foreground',
        )}
      >
        <span className="flex items-center gap-1.5">
          <Icon aria-hidden="true" className={cn('size-4 shrink-0', className)} />
          <span className="min-w-0 flex-1 truncate text-foreground">{item.name}</span>
          {rank === null ? null : (
            <span
              title="Popularity rank"
              className="shrink-0 text-xs font-normal text-muted-foreground tabular-nums"
            >
              {rank}
            </span>
          )}
        </span>
        <span className="flex flex-wrap items-center gap-x-1 pl-5.5 text-xs font-normal text-muted-foreground">
          {state.words}
          {item.kind === 'read' && item.card.in_decks && !allInDecks(target) ? (
            <span>· in your decks</span>
          ) : null}
          {differs === 0 || outcome !== undefined ? null : (
            <>
              <TriangleAlertIcon aria-hidden="true" className="size-3 text-support-partial" />
              {differs === 1 ? '1 clause differs' : `${differs} clauses differ`}
            </>
          )}
        </span>
      </button>
    </li>
  )
}

/**
 * The review's list (from `lg`): To read and To check, each headed with what is left of it, every row its state. Every
 * grid in it has one `minmax(0, 1fr)` column, so a long name is cut short instead of widening the list under the card.
 */
export function ReviewList({ items, done, shown, onPick, target, listRef }: ReviewListProps) {
  const parts = [
    { kind: 'read' as const, title: 'To read', items: items.filter((item) => item.kind === 'read') },
    { kind: 'check' as const, title: 'To check', items: items.filter((item) => item.kind === 'check') },
  ].filter((part) => part.items.length > 0)
  return (
    <nav aria-label="Cards to review" className="hidden lg:sticky lg:top-4 lg:block">
      <div
        ref={listRef}
        className="surface grid max-h-[calc(100dvh-14rem)] grid-cols-1 gap-2 overflow-y-auto overscroll-contain p-1"
      >
        {parts.map((part) => {
          const left = part.items.filter((item) => !done.has(item.id)).length
          const id = `review-part-${part.kind}`
          return (
            <section
              key={part.kind}
              aria-labelledby={id}
              data-part={part.kind}
              className="grid grid-cols-1 gap-0.5"
            >
              <h2
                id={id}
                className="sticky top-0 z-10 flex items-baseline justify-between rounded-md bg-surface px-2 pt-1.5 pb-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase"
              >
                {part.title}
                <span className="font-normal tabular-nums normal-case">
                  {left === 0 ? 'done' : `${left} left`}
                </span>
              </h2>
              <ul className="grid grid-cols-1 gap-0.5">
                {part.items.map((item) => (
                  <Row
                    key={item.id}
                    item={item}
                    outcome={done.get(item.id)}
                    current={item.id === shown}
                    target={target}
                    onPick={() => onPick(item.id)}
                  />
                ))}
              </ul>
            </section>
          )
        })}
      </div>
    </nav>
  )
}

/** The keys of the review screen, as a short list under the card. */
export function ReviewKeys({ id }: { id?: string }) {
  return (
    <dl
      id={id}
      data-slot="review-keys"
      className="grid grid-cols-[auto_1fr] items-center gap-x-3 gap-y-1.5 text-sm text-muted-foreground sm:grid-cols-[auto_1fr_auto_1fr]"
    >
      {REVIEW_KEYS.map((entry) => (
        <div key={entry.does} className="contents">
          <dt className="flex gap-1">
            {entry.keys.map((key) => (
              <Kbd key={key}>{key}</Kbd>
            ))}
          </dt>
          <dd>{entry.does}</dd>
        </div>
      ))}
    </dl>
  )
}
