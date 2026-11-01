import { Play, X } from 'lucide-react'
import { useEffect } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { Button } from '@/components/ui/button'
import { BY_HAND_TITLE } from '@/lib/byHand'
import { cn } from '@/lib/utils'
import { abilityText, costText, keywordLine, powerToughness, statusWords, typeLine } from './cardModel'
import { KeywordText } from './Keyword'
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
  return target === null ? null : <PreviewBody target={target} />
}

/** The tag after an ability the viewer can activate now, in the look of the reason tag under the card's name. */
function NowTag() {
  return (
    <span
      data-slot="preview-now"
      className="ml-1.5 inline-flex items-center gap-1 rounded-md bg-muted px-2 py-0.5 align-middle text-xs font-medium text-foreground"
    >
      <Play aria-hidden="true" className="size-3 fill-current" />
      Can activate now
    </span>
  )
}

function PreviewBody({ target }: { target: PreviewTarget }) {
  const store = usePreviewStore()
  const name = useSeatNamer()
  const { id } = target
  const object = usePlay((state) => state.objectsById[id])
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
  const gone = object === undefined

  useEffect(() => {
    // A pinned card that left the game would otherwise stay "shown" with nothing to draw.
    if (gone) store.dismiss()
    const close = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') store.dismiss()
    }
    document.addEventListener('keydown', close)
    return () => document.removeEventListener('keydown', close)
  }, [gone, store])

  if (object === undefined) return null
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
        />
      </div>
      <div className={cn('grid content-start gap-1 text-sm', beside ? 'min-w-0 flex-1' : 'mt-3')}>
        <p className="font-semibold">{roomTitle(object.name, links.doors)}</p>
        {reason === undefined ? null : (
          <p
            data-slot="preview-reason"
            className="w-fit rounded-md bg-muted px-2 py-0.5 text-xs font-medium text-foreground"
          >
            {reason}.
          </p>
        )}
        {object.mana_cost === null ? null : <p>Mana cost {costText(object.mana_cost)}</p>}
        <p>{typeLine(object)}</p>
        {pt === null ? null : <p>Power/toughness {pt}</p>}
        {keywords === null ? null : <p data-slot="preview-keywords">Keywords: {keywords}</p>}
        {object.abilities.length === 0 ? (
          activated.length === 0 && manual.length === 0 ? (
            <p className="text-muted-foreground">No abilities.</p>
          ) : null
        ) : (
          <ul aria-label="Abilities" className="grid gap-0.5">
            {object.abilities.map((ability, index) => (
              <li key={index}>
                <KeywordText text={abilityText(ability)} />
              </li>
            ))}
          </ul>
        )}
        <ManaPreviewLine object={object} />
        {manual.length === 0 ? null : (
          <div data-slot="preview-by-hand">
            <p className="font-medium">{BY_HAND_TITLE}:</p>
            <ul aria-label={BY_HAND_TITLE} className="grid gap-0.5">
              {manual.map((clause, index) => (
                <li key={index}>
                  <KeywordText text={clause.text} />
                </li>
              ))}
            </ul>
          </div>
        )}
        {activated.length === 0 ? null : (
          <ul aria-label="Activated abilities" data-slot="preview-activated" className="grid gap-0.5">
            {activated.map((line) => (
              <li key={line.key} data-now={line.now || undefined}>
                <KeywordText text={line.text} />
                {line.now ? <NowTag /> : null}
              </li>
            ))}
          </ul>
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
          <div className="sticky -bottom-3 -mb-3 bg-popover pt-1 pb-3">
            <Button variant="outline" size="sm" onClick={() => store.dismiss()}>
              <X aria-hidden="true" />
              Close preview
            </Button>
          </div>
        ) : null}
      </div>
    </div>
  )
}
