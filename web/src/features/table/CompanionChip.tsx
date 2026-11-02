/** The companion slot of a seat panel: the card's name and where it is, with the card shown on hover or focus. */
import { Users } from 'lucide-react'
import { useContext, useEffect, useState } from 'react'
import { CardImage } from '@/features/cards/CardImage'
import { cardImageUrl } from '@/features/cards/images'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import type { PlayerView } from '@/protocol/generated'
import { CardsApiContext } from './cardsContext'
import { companionFacts } from './companion'
import { CHIP } from './PodParts'

const STAGE_MARK = { Hidden: '?', Revealed: 'out', Brought: 'hand' } as const

/** The oracle id of the card called `name`, found once through the card search; null while unknown. */
function useOracleId(name: string): string | null {
  const api = useContext(CardsApiContext)
  const [found, setFound] = useState<{ name: string; id: string } | null>(null)
  useEffect(() => {
    const controller = new AbortController()
    api.search({ text: name, perPage: 5 }, controller.signal).then(
      (page) => {
        const hit = page.cards.find((card) => card.name.toLowerCase() === name.toLowerCase())
        if (hit !== undefined) setFound({ name, id: hit.oracle_id })
      },
      () => undefined,
    )
    return () => controller.abort()
  }, [api, name])
  return found !== null && found.name === name ? found.id : null
}

/** The tooltip's card: its name and, once found, its picture. */
function Shown({ name }: { name: string }) {
  const id = useOracleId(name)
  return (
    <div data-slot="companion-preview" className="grid gap-1">
      <span>{name}</span>
      {id === null ? null : (
        <CardImage
          src={cardImageUrl({ kind: 'card', identity: { oracle_id: id } }, 0, 'normal')}
          alt={name}
          className="w-44"
        />
      )}
    </div>
  )
}

/** A seat's companion, named by where it is; nothing while the seat named none. */
export function CompanionChip({ owner, player }: { owner: string; player: PlayerView }) {
  const { companion } = player
  if (companion === undefined) return null
  const facts = companionFacts(companion, owner)
  const chip = (
    <span
      role="img"
      aria-label={facts.words}
      data-slot="companion"
      data-stage={companion.stage}
      tabIndex={0}
      className={cn(CHIP, 'max-w-full bg-muted px-1.5 text-muted-foreground')}
    >
      <Users aria-hidden="true" className="size-4 shrink-0" />
      <span aria-hidden="true" className="truncate">
        {facts.label}
      </span>
      <span aria-hidden="true" className="shrink-0 opacity-70">
        {STAGE_MARK[companion.stage]}
      </span>
    </span>
  )
  if (facts.name === undefined) return chip
  return (
    <Tooltip>
      <TooltipTrigger render={chip} />
      <TooltipContent side="bottom">
        <Shown name={facts.name} />
      </TooltipContent>
    </Tooltip>
  )
}
