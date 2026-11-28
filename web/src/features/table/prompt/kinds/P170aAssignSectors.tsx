import { useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { moveToSector, SECTORS, sectorWords, spreadSectors } from '../sectorsText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus } from '../usePromptKeys'

/**
 * P170aAssignSectors (space sculptor, Space Beleren): the player assigns each of their creatures in no sector to the
 * alpha, beta or gamma sector. Each creature is a row with one toggle per sector, exactly one pressed; the rows start
 * spread over the sectors in turn, the answer the response clock gives, and Confirm has the focus. The list scrolls
 * inside the prompt on a small screen.
 */
export const P170aAssignSectorsPrompt: PromptComponent<'P170aAssignSectors'> = ({ options }) => {
  const { answer } = usePrompt()
  const objects = useObjects(useMemo(() => [...options.creatures], [options.creatures]))
  const name = (id: number): string => objects.find((object) => object?.id === id)?.name ?? 'A creature'
  const [sectors, setSectors] = useState(() => spreadSectors(options.creatures))
  const confirm = useRef<HTMLButtonElement>(null)
  useDefaultFocus(confirm)

  return (
    <PromptLayout
      text={<span data-slot="sectors-text">{options.text}</span>}
      choices={
        <div className="flex basis-full flex-col gap-2">
          <ul
            aria-label="Creatures"
            data-slot="sector-rows"
            className="flex max-h-56 flex-col gap-1.5 overflow-y-auto p-0.5"
          >
            {sectors.map(([creature, chosen]) => (
              <li key={creature} className="flex flex-wrap items-center gap-2" data-slot="sector-row">
                <span className="min-w-28 flex-1 truncate text-sm">{name(creature)}</span>
                <div role="group" aria-label={`Sector for ${name(creature)}`} className="flex flex-wrap gap-1.5">
                  {SECTORS.map((sector) => {
                    const on = sector === chosen
                    return (
                      <Button
                        key={sector}
                        type="button"
                        variant="outline"
                        className={cn('h-9 min-w-16 px-2', on && 'border-primary bg-primary/10')}
                        aria-label={`${name(creature)} in ${sector.toLowerCase()}`}
                        aria-pressed={on}
                        onClick={() => setSectors((was) => moveToSector(was, creature, sector))}
                      >
                        {sector}
                      </Button>
                    )
                  })}
                </div>
              </li>
            ))}
          </ul>
          <p className="text-sm text-muted-foreground" data-slot="sector-summary">
            {SECTORS.map((sector) =>
              sectorWords(
                sector,
                sectors.filter(([, chosen]) => chosen === sector).map(([creature]) => name(creature)),
              ),
            ).join(' · ')}
          </p>
        </div>
      }
    >
      <Button ref={confirm} className="h-10 px-4" onClick={() => void answer({ P170aAssignSectors: sectors })}>
        Confirm sectors
      </Button>
    </PromptLayout>
  )
}
