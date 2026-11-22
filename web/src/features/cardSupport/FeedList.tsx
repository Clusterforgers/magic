import { CircleAlertIcon, LayersIcon, PackagePlusIcon, SparklesIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { timeSince } from '@/lib/timeSince'
import { cn } from '@/lib/utils'
import type { FeedEvent } from '@/protocol/generated'
import { feedLine } from './model'
import type { FeedLine } from './model'

const ICON: Readonly<Record<FeedLine['kind'], { Icon: LucideIcon; tone: string }>> = {
  new_precons: { Icon: PackagePlusIcon, tone: 'text-primary' },
  new_cards: { Icon: LayersIcon, tone: 'text-support-partial' },
  now_automatic: { Icon: SparklesIcon, tone: 'text-support-verified' },
  check_failed: { Icon: CircleAlertIcon, tone: 'text-destructive' },
}

/** What changed, the newest first: new precons per set, new cards, cards now automatic and failed checks. */
export function FeedList({ feed, now }: { feed: readonly FeedEvent[]; now: number }) {
  return (
    <section aria-labelledby="feed-title" className="surface grid content-start gap-3 p-4 sm:p-6">
      <h2 id="feed-title" className="text-lg font-semibold tracking-tight">
        What changed
      </h2>
      {feed.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          Nothing yet. New sets, precons and cards that start to play automatically show up here.
        </p>
      ) : (
        <ol className="grid gap-3" data-slot="feed">
          {feed.map((event, at) => {
            const line = feedLine(event)
            const { Icon, tone } = ICON[line.kind]
            return (
              <li
                key={`${line.at}-${at}`}
                data-kind={line.kind}
                className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3"
              >
                <Icon aria-hidden="true" className={cn('mt-0.5 size-4', tone)} />
                <div className="grid min-w-0 gap-0.5">
                  <p className="text-sm font-medium break-words">{line.title}</p>
                  {line.detail === '' ? null : (
                    <p className="line-clamp-2 text-sm text-muted-foreground break-words" title={line.detail}>
                      {line.detail}
                    </p>
                  )}
                  <p className="text-xs text-muted-foreground">
                    <time dateTime={line.at}>{timeSince(line.at, now)}</time>
                  </p>
                </div>
              </li>
            )
          })}
        </ol>
      )}
    </section>
  )
}
