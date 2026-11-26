import { useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { confirmPilesLabel, emptyPiles, pileWords, togglePile } from '../pilesText'
import { KeyHint } from '../KeyHint'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/**
 * R509BlockPiles (Camouflage, CR 509.1): instead of declaring blockers the defending player divides creatures into one
 * pile per attacker. Each creature is a row with a toggle per pile; a creature that can block additional creatures may
 * be in that many piles, and further toggles turn off. The piles go to the attackers at random once confirmed, so they
 * are numbered rather than named after an attacker. No blocks (N) answers with every pile empty, the answer the
 * response clock gives, and has the focus; the list scrolls inside the prompt on a small screen.
 */
export const R509BlockPilesPrompt: PromptComponent<'R509BlockPiles'> = ({ options }) => {
  const { answer } = usePrompt()
  const ids = useMemo(
    () => [...options.attackers, ...options.creatures.map((creature) => creature.object)],
    [options.attackers, options.creatures],
  )
  const objects = useObjects(ids)
  const name = (id: number): string => objects.find((object) => object?.id === id)?.name ?? 'A creature'
  const [piles, setPiles] = useState(() => emptyPiles(options.attackers.length))
  const none = useRef<HTMLButtonElement>(null)
  useDefaultFocus(none)
  const noBlocks = (): void => void answer({ R509BlockPiles: emptyPiles(options.attackers.length) })
  usePromptKeys({ n: noBlocks })

  const attackers = options.attackers.map(name).join(', ')
  return (
    <PromptLayout
      text={
        <span data-slot="piles-text">
          {options.text} <span className="text-muted-foreground">Attacking you: {attackers}.</span>
        </span>
      }
      choices={
        <div className="flex basis-full flex-col gap-2">
          <ul aria-label="Creatures" data-slot="pile-rows" className="flex max-h-56 flex-col gap-1.5 overflow-y-auto p-0.5">
            {options.creatures.map((creature) => {
              const count = piles.filter((pile) => pile.includes(creature.object)).length
              return (
                <li key={creature.object} className="flex flex-wrap items-center gap-2" data-slot="pile-row">
                  <span className="min-w-28 flex-1 truncate text-sm">
                    {name(creature.object)}
                    {creature.piles > 1 ? (
                      <span className="text-muted-foreground"> (up to {creature.piles} piles)</span>
                    ) : null}
                  </span>
                  <div role="group" aria-label={`Piles for ${name(creature.object)}`} className="flex flex-wrap gap-1.5">
                    {piles.map((pile, at) => {
                      const on = pile.includes(creature.object)
                      return (
                        <Button
                          key={at}
                          type="button"
                          variant="outline"
                          className={cn('h-9 min-w-10 px-2 tabular-nums', on && 'border-primary bg-primary/10')}
                          aria-label={`${name(creature.object)} in pile ${at + 1}`}
                          aria-pressed={on}
                          disabled={!on && count >= creature.piles}
                          onClick={() => setPiles((was) => togglePile(was, at, creature.object, creature.piles))}
                        >
                          {at + 1}
                        </Button>
                      )
                    })}
                  </div>
                </li>
              )
            })}
          </ul>
          <p className="text-sm text-muted-foreground" data-slot="pile-summary">
            {piles.map((pile, at) => pileWords(at, pile.map(name))).join(' · ')}
          </p>
        </div>
      }
    >
      <Button className="h-10 px-4" onClick={() => void answer({ R509BlockPiles: piles })}>
        {confirmPilesLabel(piles)}
      </Button>
      <Button
        ref={none}
        variant="outline"
        className="h-10 min-w-24 px-4"
        aria-label="No blocks: every pile empty"
        aria-keyshortcuts="N"
        onClick={noBlocks}
      >
        No blocks
        <KeyHint>N</KeyHint>
      </Button>
    </PromptLayout>
  )
}
