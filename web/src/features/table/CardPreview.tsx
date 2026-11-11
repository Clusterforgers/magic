import { BookOpen, Crosshair, Play, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { ReadDialog } from '@/features/reads/ReadDialog'
import { readableOf } from '@/features/reads/readable'
import type { Reading } from '@/features/reads/readable'
import { useAuth } from '@/features/auth/context'
import { Button } from '@/components/ui/button'
import { SupportMark } from '@/features/decks/SupportMark'
import { BY_HAND_TITLE } from '@/lib/byHand'
import { READ_IT, UNVERIFIED_MEANING } from '@/lib/toRead'
import { cn } from '@/lib/utils'
import {
  abilityText,
  costPips,
  costText,
  keywordLine,
  powerToughness,
  statusWords,
  typeLine,
} from './cardModel'
import { ManaPips } from './ManaPips'
import { AbilityLine, RulesText } from './RulesText'
import { reminderTerms } from './reminders'
import { ManaPreviewLine } from './ManaStrip'
import { PreviewCard } from './PreviewCard'
import { useCreatureOutlook } from './combat/drawn'
import { outlookText } from './combat/forecastText'
import { useSeatNamer } from './names'
import { usePreviewStore, usePreviewTarget } from './preview'
import type { Dock, PreviewTarget } from './preview'
import { previewFit } from './previewFit'
import {
  activatedLines,
  attachLine,
  combatText,
  controlText,
  hostLine,
  pairLine,
  zoneText,
} from './previewText'
import { clausesOf } from './manual/clauses'
import { useCardMark } from './prompt/marks'
import { noteName, revealedLine, revealedPlace } from './revealed'
import { combatRoleIn } from './selectors'
import { roomTitle } from './rooms'
import { AutoPassButton } from './AutoPassButton'
import { isStacked, shownObject, stackSummary } from './stackPreview'
import { useStackEntries, useTargetNamer } from './useStackEntries'
import { useLinks } from './useLinks'

// Whole class names, so Tailwind can see them.
const DOCK: Record<Dock, string> = {
  right: 'right-3',
  left: 'left-3',
  top: 'inset-x-2',
  bottom: 'inset-x-2',
}

/**
 * The large view of the card under the pointer or keyboard focus, with its abilities text. It docks at the edge of
 * the screen away from its card and draws the picture up to twice the size the table gives it (`previewFit`), so it
 * never covers what it shows and the card's own text reads. A click pins it; Escape or Close dismisses it.
 * Nothing is subscribed to until a card is shown, so the idle preview costs a game no renders.
 */
export function CardPreview() {
  const target = usePreviewTarget()
  const [reading, setReading] = useState<Reading | null>(null)
  return (
    <>
      {target === null ? null : <PreviewBody target={target} onRead={setReading} />}
      <ReadDialog
        oracleId={reading?.oracleId ?? null}
        name={reading?.name ?? ''}
        onClose={() => setReading(null)}
      />
    </>
  )
}

/** The tag after an ability the viewer can activate now, in the look of the reason tag under the card's name. */
function NowTag() {
  return (
    <span
      data-slot="preview-now"
      className="ml-1.5 inline-flex items-center gap-1 rounded-md bg-muted px-2 py-0.5 align-middle text-xs font-medium text-foreground"
    >
      <Play aria-hidden="true" className="size-3 fill-current text-primary" />
      Can activate now
    </span>
  )
}

/** The small heading over a group of the preview's lines; the list under it carries the same name for a screen reader. */
function PreviewHeading({ children }: { children: string }) {
  return (
    <p
      aria-hidden="true"
      className="mt-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase"
    >
      {children}
    </p>
  )
}

function PreviewBody({ target, onRead }: { target: PreviewTarget; onRead: (card: Reading) => void }) {
  const store = usePreviewStore()
  const name = useSeatNamer()
  const { id } = target
  const held = usePlay((state) => state.objectsById[id])
  const sourceObject = usePlay((state) =>
    held?.source === undefined ? undefined : state.objectsById[held.source],
  )
  const object = held === undefined ? undefined : shownObject(held, sourceObject)
  const entry = useStackEntries().find((candidate) => candidate.id === id)
  const targetNames = useTargetNamer()
  const role = usePlay((state) => combatRoleIn(state, id))
  const place = usePlay((state) => (state.view === null ? undefined : revealedPlace(state.view.players, id)))
  const outlook = useCreatureOutlook(id)
  const fight = usePlay((state) =>
    state.view === null
      ? null
      : combatText(state.view.combat, id, name, (other) => state.objectsById[other]?.name ?? 'a card'),
  )
  const links = useLinks(object)
  const note = useCardMark(id).note
  const reason = note?.line
  const gone = held === undefined
  const signedIn = useAuth((state) => state.status === 'signed-in')

  const { pinned } = target
  useEffect(() => {
    // A pinned card that left the game would otherwise stay "shown" with nothing to draw.
    if (gone) store.dismiss()
    const close = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') return
      // A pinned preview takes the press for itself: a prompt's Undo waits for the next one.
      if (pinned) event.preventDefault()
      store.dismiss()
    }
    document.addEventListener('keydown', close)
    return () => document.removeEventListener('keydown', close)
  }, [gone, store, pinned])

  if (object === undefined || held === undefined) return null
  const summary = isStacked(held) ? stackSummary(held, sourceObject, entry, name, targetNames) : null
  const fit = previewFit(target.anchor, { width: window.innerWidth, height: window.innerHeight })
  const { dock, beside } = fit
  const pt = powerToughness(object)
  const keywords = keywordLine(object)
  const activated = activatedLines(object, note?.usable)
  const manual = clausesOf(object)
  const reminders = reminderTerms(object, [
    ...activated.map((line) => line.text),
    ...manual.map((clause) => clause.text),
  ])
  const status = statusWords(object, role)
  const hosted = hostLine(object, links.host)
  const paired = pairLine(links.partner)
  const attaches = links.attaches ?? []
  const readable = signedIn ? readableOf(object, manual.length) : null
  return (
    <div
      role="group"
      aria-label="Card preview"
      data-testid="card-preview"
      data-dock={dock}
      data-picture={fit.picture}
      data-pinned={target.pinned || undefined}
      style={{
        top: fit.top ?? undefined,
        bottom: fit.bottom ?? undefined,
        width: fit.width ?? undefined,
        maxHeight: fit.maxHeight,
      }}
      className={cn(
        'fixed z-[60] overflow-y-auto rounded-xl bg-popover p-3 text-popover-foreground shadow-lg ring-1 ring-foreground/15',
        DOCK[dock],
        beside ? 'flex gap-3' : null,
        target.pinned ? null : 'pointer-events-none',
      )}
    >
      <div className={cn('shrink-0', beside ? null : 'mx-auto w-fit')}>
        <PreviewCard
          key={id}
          object={object}
          role={role}
          size={beside ? 'medium' : 'large'}
          width={fit.picture}
          revealed={place !== undefined}
          usable={note?.usable}
        />
      </div>
      <div className={cn('grid content-start gap-1 text-sm', beside ? 'min-w-0 flex-1' : 'mt-3')}>
        <p className="font-semibold">{roomTitle(object.name, links.doors)}</p>
        {summary === null ? null : (
          <div data-slot="preview-stack" className="grid gap-0.5 rounded-md bg-muted px-2 py-1">
            <p className="font-medium">{summary.heading}</p>
            {summary.rules.map((line, index) => (
              <p key={index} data-slot="preview-stack-rules" className="select-text">
                <RulesText text={line} />
              </p>
            ))}
            {summary.targets.length === 0 ? null : (
              <ul aria-label="Targets" data-slot="preview-stack-targets" className="grid gap-0.5">
                {summary.targets.map((line, index) => (
                  <li key={index}>
                    <Crosshair aria-hidden="true" className="mr-1 inline size-3 align-[-1px] text-primary" />
                    <span className="sr-only">→ </span>
                    {line}
                  </li>
                ))}
              </ul>
            )}
            {held === undefined ? null : <AutoPassButton object={held} />}
          </div>
        )}
        {reason === undefined ? null : (
          <p
            data-slot="preview-reason"
            className="w-fit rounded-md bg-muted px-2 py-0.5 text-xs font-medium text-foreground"
          >
            {reason}.
          </p>
        )}
        {/* What the card is: cost, type and size, each fact named in bold. */}
        <dl
          data-slot="preview-facts"
          className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-2 gap-y-0.5"
        >
          {object.mana_cost === null ? null : (
            <>
              <dt className="font-semibold">
                Mana cost<span className="sr-only">: </span>
              </dt>
              <dd>
                <span className="sr-only">{costText(object.mana_cost)}</span>
                <ManaPips
                  pips={costPips(object.mana_cost)}
                  sizeClass="size-4 text-xs"
                  label="Mana cost"
                  className="[&]:inline-flex"
                />
              </dd>
            </>
          )}
          <dt className="font-semibold">
            Type<span className="sr-only">: </span>
          </dt>
          <dd>{typeLine(object)}</dd>
          {pt === null ? null : (
            <>
              <dt className="font-semibold">
                Power/toughness<span className="sr-only">: </span>
              </dt>
              <dd className="tabular-nums">{pt}</dd>
            </>
          )}
          {keywords === null ? null : (
            <>
              <dt className="font-semibold">
                Keywords<span className="sr-only">: </span>
              </dt>
              <dd data-slot="preview-keywords">{keywords}</dd>
            </>
          )}
          {object.lights === undefined || object.lights.length === 0 ? null : (
            <>
              <dt className="font-semibold">
                Lit up<span className="sr-only">: </span>
              </dt>
              <dd data-slot="preview-lights">{object.lights.join(', ')}</dd>
            </>
          )}
          {object.stickers === undefined || object.stickers.length === 0 ? null : (
            <>
              <dt className="font-semibold">
                Stickers<span className="sr-only">: </span>
              </dt>
              <dd data-slot="preview-stickers">{object.stickers.join('; ')}</dd>
            </>
          )}
        </dl>
        {object.abilities.length === 0 ? (
          activated.length === 0 && manual.length === 0 ? (
            <p className="text-muted-foreground">No abilities.</p>
          ) : null
        ) : (
          <>
            <PreviewHeading>Abilities</PreviewHeading>
            <ul aria-label="Abilities" className="grid gap-1">
              {object.abilities.map((ability, index) => (
                <li key={index}>
                  <RulesText text={abilityText(ability)} />
                </li>
              ))}
            </ul>
          </>
        )}
        <ManaPreviewLine object={object} />
        {manual.length === 0 ? null : (
          <div data-slot="preview-by-hand">
            <p className="font-medium">{BY_HAND_TITLE}:</p>
            <ul aria-label={BY_HAND_TITLE} className="grid gap-0.5">
              {manual.map((clause, index) => (
                <li key={index}>
                  <RulesText text={clause.text} />
                </li>
              ))}
            </ul>
          </div>
        )}
        {activated.length === 0 ? null : (
          <>
            <PreviewHeading>Activated abilities</PreviewHeading>
            {/* The ones offered now come first in the eye: an edge, and the words "Can activate now" after them. */}
            <ul aria-label="Activated abilities" data-slot="preview-activated" className="grid gap-1">
              {activated.map((line) => (
                <li
                  key={line.key}
                  data-now={line.now || undefined}
                  className={cn('border-s-2 ps-2', line.now ? 'border-primary' : 'border-transparent')}
                >
                  <AbilityLine text={line.text} keyword={line.keyword} />
                  {line.now ? <NowTag /> : null}
                </li>
              ))}
            </ul>
          </>
        )}
        {reminders.length === 0 ? null : (
          <ul
            aria-label="Keyword reminders"
            data-slot="preview-reminders"
            className="grid gap-0.5 text-xs text-muted-foreground"
          >
            {reminders.map((term) => (
              <li key={term.title}>
                <span className="font-medium text-foreground">{term.title}</span>: {term.text}
              </li>
            ))}
          </ul>
        )}
        {object.unverified === true ? (
          <p data-slot="preview-unverified" className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <SupportMark support="unverified" />
            <span className="text-muted-foreground">{UNVERIFIED_MEANING}.</span>
            {readable === null ? null : (
              <button
                type="button"
                data-slot="preview-unverified-read"
                className="font-medium text-primary underline-offset-2 hover:underline focus-visible:underline"
                onClick={() => {
                  onRead(readable)
                  store.dismiss()
                }}
              >
                {READ_IT}
              </button>
            )}
          </p>
        ) : null}
        {status.length === 0 ? null : <p>Status: {status.join(', ')}</p>}
        {hosted === null ? null : <p data-slot="preview-host">{hosted}.</p>}
        {paired === null ? null : <p data-slot="preview-paired">{paired}.</p>}
        {attaches.length === 0 ? null : (
          <ul aria-label="Attachments" data-slot="preview-attachments" className="grid gap-0.5">
            {attaches.map((attach) => (
              <li key={attach.id} data-foreign={attach.foreign ?? undefined}>
                {attachLine(attach, name)}
              </li>
            ))}
          </ul>
        )}
        {place === undefined ? null : (
          <p data-slot="preview-revealed">{revealedLine(place, name(object.owner))}</p>
        )}
        {object.note === undefined ? null : <p data-slot="preview-note">{noteName(object.note)}</p>}
        {fight === null ? null : <p>{fight}</p>}
        {outlook === null ? null : (
          <p data-slot="preview-forecast">Combat forecast: {outlookText(outlook)}.</p>
        )}
        <p className="text-muted-foreground">
          {zoneText(object.zone, name)}. {controlText(object, name)}.
        </p>
        {target.pinned ? (
          // Stays at the foot of the panel while the lines above it scroll.
          <div className="sticky -bottom-3 -mb-3 flex flex-wrap gap-2 bg-popover pt-1 pb-3">
            <Button variant="outline" size="sm" onClick={() => store.dismiss()}>
              <X aria-hidden="true" />
              Close preview
            </Button>
            {readable === null ? null : (
              <Button
                variant="outline"
                size="sm"
                data-slot="preview-read"
                onClick={() => {
                  onRead(readable)
                  store.dismiss()
                }}
              >
                <BookOpen aria-hidden="true" />
                Read its rules
              </Button>
            )}
          </div>
        ) : null}
      </div>
    </div>
  )
}
