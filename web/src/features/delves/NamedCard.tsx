/**
 * A real card a delve offers by name (a loot pick or a shop ware): its picture, found once per name through the card
 * search, else a framed tile with the name while it loads or where the search has no picture.
 */
import { SparklesIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { cardImageUrl } from '@/features/cards/images'
import { cn } from '@/lib/utils'
import { searchByName } from './findCard'
import type { FindCard } from './findCard'

const defaultFind = searchByName()

export function NamedCard({
  name,
  find = defaultFind,
  sold = false,
  className,
  children,
}: {
  name: string
  find?: FindCard
  /** A ware already bought: drawn faded with "Sold". */
  sold?: boolean
  className?: string
  /** Controls under the card, such as a Take button. */
  children?: ReactNode
}) {
  const [oracle, setOracle] = useState<string | null>(null)
  const [missed, setMissed] = useState(false)
  useEffect(() => {
    let live = true
    void find(name).then((id) => live && setOracle(id))
    return () => {
      live = false
    }
  }, [find, name])
  const picture = oracle !== null && !missed
  return (
    <figure
      data-slot="named-card"
      aria-label={name}
      className={cn('grid w-44 max-w-full content-start gap-2', sold && 'opacity-60', className)}
    >
      <div className="relative aspect-[5/7] overflow-hidden rounded-[4.75%/3.393%] border bg-raised shadow-e1">
        {picture ? (
          <img
            src={cardImageUrl({ kind: 'card', identity: { oracle_id: oracle } }, 0, 'normal')}
            alt=""
            loading="lazy"
            onError={() => setMissed(true)}
            className="size-full object-cover"
          />
        ) : (
          <span className="grid size-full place-items-center gap-2 p-3 text-center">
            <SparklesIcon aria-hidden="true" className="size-6 text-delve" />
          </span>
        )}
        {sold ? (
          <span className="absolute inset-x-0 top-1/2 -translate-y-1/2 bg-raised py-1 text-center text-sm font-semibold">
            Sold
          </span>
        ) : null}
      </div>
      <figcaption className="text-sm leading-tight font-medium">{name}</figcaption>
      {children}
    </figure>
  )
}
