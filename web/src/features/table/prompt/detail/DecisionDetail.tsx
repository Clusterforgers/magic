import { useContext } from 'react'
import type { ReactNode } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectView, RunEntryView } from '@/protocol/generated'
import { CardFace } from '../../CardFace'
import { RulesText } from '../../RulesText'
import { SideContext } from '../../sideContext'
import { usePreviewHandlers } from '../../usePreviewHandlers'
import { useOracleText } from '../oracleWords'
import { ownDecision } from '../ownDecision'
import { ArtDisc } from './AskerChip'
import { askKicker, clauseParts, kindKey, mayClause, runRowWords } from './asking'
import { lightHandlers } from './boardBadges'
import { outcomeOf } from './runMemory'
import { useAsker } from './useAsker'
import type { ShownAsker } from './useAsker'

/** A kicker line: its lead in the accent, the rest after a dot. */
export function Kicker({ lead, rest }: { lead: string; rest?: string }) {
  return (
    <p data-slot="detail-kicker" className="detail-kicker">
      <b>{lead}</b>
      {rest === undefined || rest === '' ? null : ` · ${rest}`}
    </p>
  )
}

/** Rules text with the asked clause marked. */
function MarkedRules({ text, clause }: { text: string; clause: readonly [number, number] | undefined }) {
  const { before, marked, after } = clauseParts(text, clause)
  return (
    <p data-slot="detail-rules" className="text-[0.8125rem] leading-[1.1875rem] text-muted-foreground">
      {before === '' ? null : <RulesText text={before} />}
      {marked === '' ? null : (
        <mark data-slot="asked-clause" className="detail-clause">
          <RulesText text={marked} />
        </mark>
      )}
      {after === '' ? null : <RulesText text={after} />}
    </p>
  )
}

/** The words of the asking card: the ability's text from the server, else the card's own text with its "you may" found here. */
function useAskedText(asker: ShownAsker): { text: string; clause: readonly [number, number] | undefined } | null {
  const question = usePlay((state) => {
    const kind = ownDecision(state)?.decision.kind
    return kind !== undefined && 'YesNo' in kind ? (kind.YesNo.text ?? '') : null
  })
  // The card's own text is read only where the server sent none (a spell).
  const own = useOracleText(asker.text === undefined ? asker.object : undefined)
  if (asker.text !== undefined) return { text: asker.text, clause: asker.clause }
  if (own === null) return null
  return { text: own, clause: question === null ? undefined : mayClause(own, question) }
}

/** The asking card as a card frame with its tag, name and rules text. */
function AskEntry({ asker, tag }: { asker: ShownAsker; tag: string }) {
  const words = useAskedText(asker)
  const preview = usePreviewHandlers(asker.object?.id ?? -1)
  const light = lightHandlers(asker.board)
  return (
    <div
      data-slot="ask-entry"
      data-asker={asker.board ?? asker.object?.id}
      className="detail-entry"
      onPointerEnter={(event) => {
        light.onPointerEnter?.()
        if (asker.object !== undefined) preview.onPointerEnter(event)
      }}
      onPointerLeave={() => {
        light.onPointerLeave?.()
        preview.onPointerLeave()
      }}
    >
      <span
        aria-hidden="true"
        data-slot="ask-card"
        className="relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center rounded-lg shadow-e2 [--card-h:8rem] [--card-w:5.75rem] in-data-[where=sheet]:[--card-h:5.75rem] in-data-[where=sheet]:[--card-w:4.125rem]"
      >
        {asker.object === undefined ? (
          <span className="grid size-full place-items-center rounded-md border-2 border-border-strong bg-muted text-xs text-muted-foreground">
            Card
          </span>
        ) : (
          <CardFace object={asker.object} role={null} size="normal" fit />
        )}
      </span>
      <div className="grid min-w-0 content-start gap-1.5">
        <span data-slot="ask-tag" className="detail-tag">
          {tag}
        </span>
        <b className="text-sm leading-[1.125rem] font-semibold">{asker.shown === '' ? 'A card' : asker.shown}</b>
        {words === null ? null : <MarkedRules text={words.text} clause={words.clause} />}
      </div>
    </div>
  )
}

/** A done or waiting trigger of the run: its number or tick, its picture, its name and what it did or does. */
function RunRow({
  entry,
  place,
  index,
  outcome,
}: {
  entry: RunEntryView
  place: number
  index: number
  outcome: string | undefined
}) {
  const object = usePlay((state): ObjectView | undefined =>
    entry.source === undefined ? undefined : state.objectsById[entry.source],
  )
  const done = place < index - 1
  const name = entry.name ?? object?.name ?? 'A trigger'
  return (
    <li
      data-slot="run-row"
      data-plain=""
      data-done={done || undefined}
      className="detail-row"
      {...lightHandlers(object?.id)}
    >
      <span className="detail-number" data-tone={done ? 'done' : undefined}>
        {done ? '✓' : place + 1}
      </span>
      <ArtDisc object={object} name={name} className="size-8 rounded-md" />
      <span className="grid min-w-0">
        <span className="truncate text-[0.8125rem] leading-[1.125rem] font-semibold">{name}</span>
        <span
          className={cn('truncate text-xs leading-4', done ? 'text-success' : 'text-muted-foreground')}
          title={runRowWords(entry, place, index, outcome)}
        >
          <RulesText text={runRowWords(entry, place, index, outcome)} />
        </span>
      </span>
      <span />
    </li>
  )
}

/** What asks: the kicker, then the asking card, inside its run when it is one of several triggers. */
function Asking({ asker }: { asker: ShownAsker }) {
  const { lead, rest } = askKicker(asker)
  const run = asker.run
  return (
    <>
      <Kicker lead={lead} rest={rest} />
      {run === undefined ? (
        <AskEntry asker={asker} tag="Asking you" />
      ) : (
        <ol
          data-slot="run"
          aria-label={`Your triggers, ${run.index} of ${run.entries.length}`}
          className="grid list-none gap-1.5"
        >
          {run.entries.map((entry, place) =>
            place === run.index - 1 ? (
              <li key={place}>
                <AskEntry asker={asker} tag={`Asking · ${run.index} of ${run.entries.length}`} />
              </li>
            ) : (
              <RunRow
                key={place}
                entry={entry}
                place={place}
                index={run.index}
                outcome={outcomeOf(run.id, place + 1)}
              />
            ),
          )}
        </ol>
      )}
    </>
  )
}

/** The decisions whose own choices open the detail, so the asking card is not drawn above them. */
const OWN_HEAD: ReadonlySet<string> = new Set(['PayMana'])

/**
 * The detail of the decision this seat must answer, for the Stack column and the sheet over the hand: the asking card
 * with its rules text and the asked clause, the run of triggers around it, and under them the element the prompt draws
 * what it chooses among into (`PromptLayout` portals its choices here). `where` sizes the card.
 */
export function DecisionDetail({ where, children }: { where: 'column' | 'sheet'; children?: ReactNode }) {
  const side = useContext(SideContext)
  const asker = useAsker()
  const kind = usePlay((state) => {
    const own = ownDecision(state)
    return own === null ? '' : kindKey(own.decision.kind)
  })
  return (
    <section
      data-slot="decision-detail"
      data-where={where}
      aria-label="Your decision"
      className="grid min-w-0 content-start gap-2"
    >
      {asker === null || OWN_HEAD.has(kind) ? null : <Asking asker={asker} />}
      <div
        ref={side?.setDetail}
        data-slot="decision-choices"
        data-decision={kind}
        className="grid min-w-0 content-start gap-2 empty:hidden"
      />
      {children}
    </section>
  )
}
