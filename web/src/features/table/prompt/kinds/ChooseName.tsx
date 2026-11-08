import { useContext, useEffect, useId, useMemo, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import type { CardHit } from '@/protocol/generated'
import { CardsApiContext } from '../../cardsContext'
import { askerWords, optionSentence } from '../choiceText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus } from '../usePromptKeys'

/** How long typing pauses before the catalog is searched. */
const SEARCH_DELAY_MS = 250

/** The hits that may be named: a land is out when the card says "nonland". */
export function nameable(hits: CardHit[], nonland: boolean): CardHit[] {
  return nonland ? hits.filter((hit) => !/\bLand\b/.test(hit.type_line)) : hits
}

/**
 * ChooseName: any card of the game may be named (CR 201.3), so the prompt searches the whole catalog by what is typed
 * and answers with the name of the hit chosen. The list holds at most eight hits; a card that says "nonland" leaves
 * out lands. "Name no card" answers with an empty name, which is what the response clock gives.
 */
export const ChooseNamePrompt: PromptComponent<'ChooseName'> = ({ options }) => {
  const { answer } = usePrompt()
  const api = useContext(CardsApiContext)
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const [typed, setTyped] = useState('')
  const [hits, setHits] = useState<CardHit[]>([])
  const [failed, setFailed] = useState(false)
  const field = useRef<HTMLInputElement>(null)
  useDefaultFocus(field)
  const fieldId = useId()
  const nonland = options.nonland === true

  useEffect(() => {
    const text = typed.trim()
    if (text === '') {
      setHits([])
      return
    }
    const controller = new AbortController()
    const timer = setTimeout(() => {
      api.search({ text, perPage: 24 }, controller.signal).then(
        (page) => {
          setFailed(false)
          setHits(nameable(page.cards, nonland).slice(0, 8))
        },
        () => {
          if (!controller.signal.aborted) setFailed(true)
        },
      )
    }, SEARCH_DELAY_MS)
    return () => {
      clearTimeout(timer)
      controller.abort()
    }
  }, [api, typed, nonland])

  const name = (card: string): void => void answer({ ChooseName: card })
  const submit = (event: FormEvent): void => {
    event.preventDefault()
    // Enter takes the card typed exactly, else the only hit.
    const text = typed.trim().toLowerCase()
    const hit =
      hits.find((card) => card.name.toLowerCase() === text) ?? (hits.length === 1 ? hits[0] : undefined)
    if (hit !== undefined) name(hit.name)
  }

  return (
    <PromptLayout text={optionSentence(askerWords(source?.name), options.text ?? 'Name a card.')}>
      <form className="flex basis-full flex-wrap items-center gap-2" data-slot="name-form" onSubmit={submit}>
        <label htmlFor={fieldId} className="sr-only">
          Card name
        </label>
        <Input
          ref={field}
          id={fieldId}
          type="search"
          autoComplete="off"
          placeholder="Search for a card"
          className="h-10 min-w-48 flex-1"
          value={typed}
          onChange={(event) => setTyped(event.target.value)}
        />
        <Button type="button" variant="outline" className="h-10 px-4" onClick={() => name('')}>
          Name no card
        </Button>
        {failed ? <span role="alert">The card search is not available.</span> : null}
        <div
          role="group"
          aria-label="Cards"
          data-slot="name-hits"
          className="flex basis-full flex-wrap gap-2"
        >
          {hits.map((hit) => (
            <Button
              key={hit.oracle_id}
              type="button"
              variant="outline"
              className="h-10 px-4"
              onClick={() => name(hit.name)}
            >
              {hit.name}
            </Button>
          ))}
        </div>
      </form>
    </PromptLayout>
  )
}
