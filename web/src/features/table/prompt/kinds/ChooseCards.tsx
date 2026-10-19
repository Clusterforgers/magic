import { useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { FocusEvent, KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { rovingTarget } from '../../useRoving'
import {
  FILTER_FROM,
  boundsOf,
  cardKeysWords,
  cardsAsk,
  cardsProgress,
  cardsRule,
  cardsSpoken,
  cardsTitle,
  countFits,
  distinctCards,
  matchesFilter,
  pressedCard,
} from '../cardsText'
import { askerWords, numberKey } from '../choiceText'
import { uniqueNames } from '../combatText'
import { markCards, useCardMarks } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptCard } from '../PromptCard'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/** Up to this many cards on offer are drawn with their cost and type line; a longer list uses the compact face, so a row is more cards. */
const NORMAL_UP_TO = 5

const NONE: readonly ObjectId[] = []

/** The card a focused element belongs to, or null. */
function candidateOf(element: EventTarget | null): ObjectId | null {
  if (!(element instanceof Element)) return null
  const raw = element.closest('[data-candidate]')?.getAttribute('data-candidate')
  if (raw === null || raw === undefined || raw.trim() === '') return null
  const id = Number(raw)
  return Number.isInteger(id) ? id : null
}

/**
 * ChooseCards: cards picked from a list (a discard, a sacrifice, a search). The cards on offer are drawn from the
 * views, in a grid that scrolls inside itself, and are chosen by a click, Enter or Space, or the number keys; the
 * cards of the table that are on offer are marked the way targets are. With room for one a press on another card
 * changes the choice, with room for several a full choice closes the rest, and Confirm waits until the number is
 * legal. A long list (a search) gets a filter box. The choice goes in the order it was made. The server's answer at
 * the deadline is the first `min` cards of the list, so focus starts on the first card.
 */
export const ChooseCardsPrompt: PromptComponent<'ChooseCards'> = ({ options }) => {
  const { answer, selection } = usePrompt()
  const own = useOwnCardIds()
  const from = useMemo(() => distinctCards(options.from), [options.from])
  const total = from.length
  const { min, max } = options
  const bounds = useMemo(() => boundsOf(min, max, total), [min, max, total])
  const { hi } = bounds

  const known = useObjects(useMemo(() => [options.source, ...from], [options.source, from]))
  const source = known[0]
  const byId = useMemo(() => new Map(from.map((id, place) => [id, known[place + 1]] as const)), [from, known])
  const labels = useMemo(
    () => uniqueNames(from.map((id) => ({ id, name: byId.get(id)?.name ?? 'A card' }))),
    [from, byId],
  )

  const fromSet = useMemo(() => new Set(from), [from])
  const chosen = useMemo(() => [...selection.ids].filter((id) => fromSet.has(id)), [selection.ids, fromSet])
  // What is chosen including presses the next render has not shown yet, so two quick presses cannot pass the most.
  const latest = useRef<readonly ObjectId[]>(chosen)
  useLayoutEffect(() => {
    latest.current = chosen
  })
  const press = (id: ObjectId): void => {
    if (!fromSet.has(id)) return
    const next = pressedCard(latest.current, id, hi)
    if (next === latest.current) return
    latest.current = next
    selection.set(next)
  }

  const closed = hi > 1 && chosen.length >= hi
  const open = hi === 0 || closed ? NONE : from
  const marks = useMemo(() => markCards(own, open, chosen), [own, open, chosen])
  useCardMarks(marks, press, { toggles: true })

  const [filter, setFilter] = useState('')
  const filtering = total > FILTER_FROM
  const visible = useMemo(
    () => (filtering ? from.filter((id) => matchesFilter(byId.get(id), filter)) : from),
    [filtering, from, byId, filter],
  )
  // Tab reaches one card of the list (the last one focused, else the first); the arrow keys move along it.
  const [last, setLast] = useState<ObjectId | null>(null)
  const current = last !== null && visible.includes(last) ? last : (visible[0] ?? null)
  const first = useRef<HTMLButtonElement>(null)
  useDefaultFocus(first)
  const onFocus = (event: FocusEvent<HTMLElement>): void => {
    const id = candidateOf(event.target)
    if (id !== null) setLast(id)
  }
  const onKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    if (event.altKey || event.ctrlKey || event.metaKey) return
    const cards = [...event.currentTarget.querySelectorAll<HTMLElement>('button[data-candidate]')]
    const index = cards.findIndex((card) => card === document.activeElement)
    const to = rovingTarget(event.key, Math.max(index, 0), cards.length)
    if (index < 0 || to === null) return
    event.preventDefault()
    cards[to]?.focus()
  }
  usePromptKeys(
    Object.fromEntries(visible.slice(0, 10).map((id, place) => [numberKey(place) ?? '', () => press(id)])),
  )

  const legal = countFits(chosen.length, bounds)
  const asker = askerWords(source?.name)
  const words = options.text ?? ''
  const text =
    chosen.length === 0 ? (
      <>
        {cardsAsk(asker, words === '' ? cardsTitle(bounds, total) : words)}
        <span className="sr-only">
          {' '}
          {cardsSpoken(words === '' ? null : cardsRule(bounds, total), cardKeysWords(visible.length))}
        </span>
      </>
    ) : (
      cardsProgress(asker, chosen.length, bounds)
    )
  const size = total <= NORMAL_UP_TO ? 'normal' : 'compact'
  const progress = total > 0 && hi > 0 ? ` · ${chosen.length} chosen` : ''

  return (
    <PromptLayout text={text}>
      <Button
        className="h-9 px-4"
        disabled={!legal}
        onClick={() => {
          if (legal) answer({ ChooseCards: [...chosen] })
        }}
      >
        Confirm
      </Button>
      <p
        data-slot="cards-count"
        aria-hidden="true"
        className="text-xs font-medium text-muted-foreground tabular-nums"
      >
        {cardsRule(bounds, total)}
        {progress}
      </p>
      {filtering ? (
        <Input
          type="search"
          aria-label="Filter cards"
          placeholder="Filter by name or type"
          autoComplete="off"
          value={filter}
          className="h-9 min-w-40 flex-1 basis-40"
          onChange={(event) => setFilter(event.target.value)}
        />
      ) : null}
      <div
        role="group"
        aria-label="Cards to choose from"
        data-slot="card-choices"
        onFocus={onFocus}
        onKeyDown={onKeyDown}
        className={cn(
          // The sheet the prompt sits in is at most `--sheet-max` high: what the stack, the sentence, the buttons and the
          // clock leave is the grid's, and it scrolls inside.
          'flex basis-full flex-wrap content-start gap-1.5 overflow-y-auto p-1.5',
          filtering
            ? 'max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_14.5rem))]'
            : 'max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_10.5rem))]',
        )}
      >
        {visible.map((id, place) => {
          const key = numberKey(place)
          return (
            <PromptCard
              key={id}
              ref={place === 0 ? first : undefined}
              id={id}
              label={labels.get(id) ?? 'A card'}
              size={size}
              chosen={chosen.includes(id)}
              closed={closed && !chosen.includes(id)}
              tabbable={id === current}
              hotkey={key}
              onPress={press}
            />
          )
        })}
        {visible.length === 0 && total > 0 ? (
          <p data-slot="no-match" className="p-2 text-sm text-muted-foreground">
            No card matches that filter.
          </p>
        ) : null}
      </div>
    </PromptLayout>
  )
}
