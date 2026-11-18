import { useEffect, useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { useGame } from '@/features/game/gameContext'
import type { ObjectView, TriggerEntry } from '@/protocol/generated'
import { orderRule } from '../../autoSkips'
import { RulesText } from '../../RulesText'
import { useCoarsePointer } from '../../useCoarsePointer'
import { ArtDisc, LeadChip } from '../detail/AskerChip'
import { lightHandlers, publishBadges } from '../detail/boardBadges'
import type { BoardBadge } from '../detail/boardBadges'
import { Kicker } from '../detail/DecisionDetail'
import {
  answerOf,
  defaultOrder,
  isDefaultOrder,
  moved,
  orderSentence,
  placeWords,
  pressSentence,
  triggerLabels,
  whichWord,
} from '../orderText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'

type Move = 'up' | 'down'

/** What a row says under the name: what the trigger does, else what it waits for. */
const effectOf = (entry: TriggerEntry): string => entry.text ?? entry.when ?? ''

/** The number badges of the triggers' sources on the board, in the order they resolve; a source keeps its first. */
function badgesOf(entries: readonly TriggerEntry[], order: readonly number[]): BoardBadge[] {
  const seen = new Set<number>()
  return order.flatMap((entry, place): BoardBadge[] => {
    const id = entries[entry]?.source.id
    if (id === undefined || seen.has(id)) return []
    seen.add(id)
    return [{ id, label: String(place + 1), tone: 'placed' }]
  })
}

/** A row's picture, name and effect. */
function RowWords({
  source,
  name,
  effect,
  first,
}: {
  source: ObjectView | undefined
  name: string
  effect: string
  first: boolean
}) {
  return (
    <>
      <ArtDisc object={source} name={name} className="size-8 rounded-md" />
      <span className="grid min-w-0">
        <span className="flex min-w-0 items-center gap-1.5 text-[0.8125rem] leading-[1.125rem] font-semibold">
          <span className="truncate">{name}</span>
          {first ? (
            <em
              data-slot="first-tag"
              className="shrink-0 rounded-full bg-turn-you px-1.5 text-xs leading-4 font-bold tracking-wider text-on-turn-you uppercase not-italic"
            >
              first
            </em>
          ) : null}
        </span>
        {effect === '' ? null : (
          <span className="line-clamp-2 text-xs leading-4 text-muted-foreground">
            <RulesText text={effect} />
          </span>
        )}
      </span>
    </>
  )
}

/**
 * OrderTriggers: the triggers are rows of the Stack column in the order they resolve, the top one first, each with its
 * number, its source's picture and name and what it does, with the engine's own order (the last to trigger resolves
 * first) in place. With a mouse a row is dragged or moved by its arrows, which keep focus; on a touch screen the rows
 * are pressed in the order they should resolve and take numbers, the last one by itself. The band holds Default order
 * (Start over on touch) and Confirm order, which sends the list as the order they go on the stack. The sources carry
 * the same numbers on the board. "Always use this order" asks the server to put the same set of triggers on in the
 * confirmed order for the rest of the game, unasked.
 */
export const OrderTriggersPrompt: PromptComponent<'OrderTriggers'> = ({ options }) => {
  const { answer } = usePrompt()
  const { session } = useGame()
  const touch = useCoarsePointer()
  const [always, setAlways] = useState(false)
  const entries = options.triggers
  const count = entries.length
  const sources = useObjects(useMemo(() => entries.map((entry) => entry.source.id), [entries]))
  const names = useMemo(() => sources.map((source) => source?.name), [sources])
  const labels = useMemo(() => triggerLabels(entries, names), [entries, names])
  // A rule keys each trigger by its source's name, so it is offered only when every source has one.
  const keyed = orderRule(entries, names, defaultOrder(count)) !== null
  // The trigger at each place of the list, by its index among the options.
  const [shown, setShown] = useState<readonly number[]>(() => defaultOrder(count))
  // On touch: the triggers pressed so far, the first to resolve first.
  const [picks, setPicks] = useState<readonly number[]>([])
  const [note, setNote] = useState<string | null>(null)
  const [dragged, setDragged] = useState<number | null>(null)
  const pressed = useRef<{ entry: number; move: Move } | null>(null)

  const move = (place: number, by: -1 | 1): void => {
    const entry = shown[place]
    if (entry === undefined) return
    pressed.current = { entry, move: by === -1 ? 'up' : 'down' }
    setShown(moved(shown, place, by))
    setNote(`${labels[entry] ?? 'The trigger'} is ${placeWords(place + by, count)}.`)
  }
  /** Drops the dragged trigger at `place`, the others closing up. */
  const drop = (place: number): void => {
    if (dragged === null) return
    const from = shown.indexOf(dragged)
    setDragged(null)
    if (from < 0 || from === place) return
    const next = shown.filter((entry) => entry !== dragged)
    next.splice(place, 0, dragged)
    setShown(next)
    setNote(`${labels[dragged] ?? 'The trigger'} is ${placeWords(place, count)}.`)
  }
  // A moved row is a new place in the page, which can drop focus; it goes back to the button just pressed. A button at
  // an end stays focusable (it only reads as unavailable), so pressing it again changes nothing instead of sending the
  // trigger back the other way.
  useEffect(() => {
    const last = pressed.current
    pressed.current = null
    if (last === null) return
    document
      .querySelector<HTMLButtonElement>(`button[data-entry="${last.entry}"][data-move="${last.move}"]`)
      ?.focus()
  }, [shown])

  const pick = (entry: number): void => {
    setPicks((now) => {
      if (now.includes(entry)) return now.filter((other) => other !== entry)
      const next = [...now, entry]
      // The last one has only one place left.
      const rest = entries.map((_, index) => index).filter((index) => !next.includes(index))
      return rest.length === 1 ? [...next, ...rest] : next
    })
  }
  const order = touch ? picks : shown
  const complete = order.length === count
  // What triggered them all, when it is one object: it carries its badge on the board and leads the band.
  const cause = entries.every((entry) => entry.cause === entries[0]?.cause) ? entries[0]?.cause : undefined
  const [causeObject] = useObjects(useMemo(() => (cause === undefined ? [] : [cause]), [cause]))
  const entered = entries.some((entry) => (entry.when ?? '').includes(' enters'))
  useEffect(() => {
    publishBadges([
      ...badgesOf(entries, order),
      ...(cause === undefined
        ? []
        : [{ id: cause, label: entered ? 'just entered' : 'cause', tone: 'cause' as const }]),
    ])
  }, [entries, order, cause, entered])
  useEffect(() => () => publishBadges([]), [])

  const confirm = (): void => {
    if (!complete) return
    const sent = answerOf(order)
    const rule = always ? orderRule(entries, names, sent) : null
    if (rule !== null) session.autoSkip(rule)
    answer({ OrderTriggers: sent })
  }

  const rows = touch ? (
    <ol
      aria-label="Your triggers; press them in the order they resolve"
      data-slot="trigger-order"
      data-mode="press"
      className="grid list-none gap-1.5"
    >
      {entries.map((entry, index) => {
        const place = picks.indexOf(index)
        const label = labels[index] ?? 'A trigger'
        const name = names[index] ?? 'A card'
        return (
          <li key={index}>
            <button
              type="button"
              data-slot="trigger-row"
              data-entry={index}
              data-interactive=""
              data-picked={place >= 0 || undefined}
              aria-pressed={place >= 0}
              aria-label={
                place >= 0 ? `${label}, resolves ${whichWord(place, count)}. Press to take it out.` : label
              }
              className="detail-row outline-offset-2 outline-primary focus-visible:outline-2"
              onClick={() => pick(index)}
              {...lightHandlers(sources[index]?.id)}
            >
              <span className="detail-number" data-tone={place >= 0 ? 'placed' : 'open'}>
                {place >= 0 ? place + 1 : ''}
              </span>
              <RowWords source={sources[index]} name={name} effect={effectOf(entry)} first={place === 0} />
              <span />
            </button>
          </li>
        )
      })}
    </ol>
  ) : (
    <ol
      aria-label="Your triggers, the one that resolves first on top"
      data-slot="trigger-order"
      data-mode="move"
      className="grid list-none gap-1.5"
    >
      {shown.map((entry, place) => {
        const label = labels[entry] ?? 'A trigger'
        const name = names[entry] ?? 'A card'
        const one = entries[entry]
        return (
          <li
            key={entry}
            draggable
            data-slot="trigger-row"
            data-entry={entry}
            data-interactive=""
            data-dragging={dragged === entry || undefined}
            className="detail-row cursor-grab"
            onDragStart={(event) => {
              event.dataTransfer.effectAllowed = 'move'
              event.dataTransfer.setData('text/plain', String(entry))
              setDragged(entry)
            }}
            onDragEnd={() => setDragged(null)}
            onDragOver={(event) => {
              if (dragged !== null) event.preventDefault()
            }}
            onDrop={(event) => {
              event.preventDefault()
              drop(place)
            }}
            {...lightHandlers(sources[entry]?.id)}
          >
            <span className="detail-number" data-tone="placed">
              <span className="sr-only">{`${place + 1}. `}</span>
              <span aria-hidden="true">{place + 1}</span>
            </span>
            <RowWords
              source={sources[entry]}
              name={name}
              effect={one === undefined ? '' : effectOf(one)}
              first={place === 0}
            />
            <span className="grid gap-0.5">
              {(['up', 'down'] as const).map((way) => {
                const end = way === 'up' ? place === 0 : place === count - 1
                return (
                  <button
                    key={way}
                    type="button"
                    data-entry={entry}
                    data-move={way}
                    aria-label={`Move ${label} ${way}`}
                    // Not `disabled`: a button that goes dead under the keyboard would drop its focus.
                    aria-disabled={end}
                    onClick={() => {
                      if (!end) move(place, way === 'up' ? -1 : 1)
                    }}
                    className="grid h-[1.375rem] w-[1.875rem] place-items-center rounded-md border text-xs text-muted-foreground outline-offset-2 outline-primary hover:bg-accent hover:text-foreground focus-visible:outline-2 aria-disabled:pointer-events-none aria-disabled:opacity-30 pointer-coarse:h-8 pointer-coarse:w-11"
                  >
                    <span aria-hidden="true">{way === 'up' ? '▲' : '▼'}</span>
                  </button>
                )
              })}
            </span>
          </li>
        )
      })}
    </ol>
  )

  return (
    <PromptLayout
      lead={
        causeObject === undefined ? undefined : (
          <LeadChip object={causeObject} name={causeObject.name} words={`Order ${count} triggers`} />
        )
      }
      text={touch ? pressSentence(count, picks.length) : orderSentence(count, note)}
      choices={
        <>
          <Kicker
            lead={`Order ${count} triggers`}
            rest={
              touch
                ? `press the one that resolves ${whichWord(Math.min(picks.length, count - 1), count)}`
                : 'top resolves first'
            }
          />
          {rows}
          {keyed ? (
            <label className="flex h-8 items-center gap-2 text-sm">
              <input
                type="checkbox"
                data-slot="always-order"
                className="size-4 accent-primary"
                checked={always}
                onChange={(event) => setAlways(event.target.checked)}
              />
              Always use this order
            </label>
          ) : null}
        </>
      }
    >
      <Button className="h-10 px-4" disabled={!complete} onClick={confirm}>
        Confirm order
      </Button>
      {touch ? (
        <Button
          variant="outline"
          className="h-10 px-4"
          disabled={picks.length === 0}
          onClick={() => setPicks([])}
        >
          Start over
        </Button>
      ) : (
        <Button
          variant="outline"
          className="h-10 px-4"
          disabled={isDefaultOrder(shown)}
          onClick={() => {
            setShown(defaultOrder(count))
            setNote(null)
          }}
        >
          Default order
        </Button>
      )}
    </PromptLayout>
  )
}
