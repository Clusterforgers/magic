import { X } from 'lucide-react'
import { useEffect } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { abilityText, costText, keywordLine, powerToughness, statusWords, typeLine } from './cardModel'
import { ManaPreviewLine } from './ManaStrip'
import { PreviewCard } from './PreviewCard'
import { useCreatureOutlook } from './combat/drawn'
import { outlookText } from './combat/forecastText'
import { useSeatNamer } from './names'
import { dockOf, usePreviewStore, usePreviewTarget } from './preview'
import type { Dock, PreviewTarget } from './preview'
import { attachLine, combatText, controlText, hostLine, zoneText } from './previewText'
import { useCardMark } from './prompt/marks'
import { noteName, revealedLine, revealedPlace } from './revealed'
import { combatRoleIn } from './selectors'
import { useLinks } from './useLinks'

// Whole class names, so Tailwind can see them.
const DOCK: Record<Dock, string> = {
  right: 'top-16 right-3 w-72',
  left: 'top-16 left-3 w-72',
  top: 'inset-x-2 top-14 flex gap-3',
  bottom: 'inset-x-2 bottom-16 flex gap-3',
}

/**
 * The large view of the card under the pointer or keyboard focus, with its abilities text. It docks at the edge of
 * the screen away from its card, so it never covers what it shows. A click pins it; Escape or Close dismisses it.
 * Nothing is subscribed to until a card is shown, so the idle preview costs a game no renders.
 */
export function CardPreview() {
  const target = usePreviewTarget()
  return target === null ? null : <PreviewBody target={target} />
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
  const reason = useCardMark(id).note?.line
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
  const dock = dockOf(target.anchor, { width: window.innerWidth, height: window.innerHeight })
  const wide = dock === 'left' || dock === 'right'
  const pt = powerToughness(object)
  const keywords = keywordLine(object)
  const status = statusWords(object, role)
  const hosted = hostLine(object, links.host)
  const attaches = links.attaches ?? []
  return (
    <div
      role="group"
      aria-label="Card preview"
      data-testid="card-preview"
      data-dock={dock}
      data-pinned={target.pinned || undefined}
      className={cn(
        'fixed z-[60] max-h-[calc(100dvh-5rem)] overflow-y-auto rounded-xl bg-popover p-3 text-popover-foreground shadow-lg ring-1 ring-foreground/15',
        DOCK[dock],
        target.pinned ? null : 'pointer-events-none',
      )}
    >
      <div className="shrink-0">
        <PreviewCard
          key={id}
          object={object}
          role={role}
          size={wide ? 'large' : 'medium'}
          revealed={place !== undefined}
        />
      </div>
      <div className={cn('grid content-start gap-1 text-sm', wide ? 'mt-3' : 'min-w-0 flex-1')}>
        <p className="font-semibold">{object.name}</p>
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
          <p className="text-muted-foreground">No abilities.</p>
        ) : (
          <ul aria-label="Abilities" className="grid gap-0.5">
            {object.abilities.map((ability, index) => (
              <li key={index}>{abilityText(ability)}</li>
            ))}
          </ul>
        )}
        <ManaPreviewLine object={object} />
        {status.length === 0 ? null : <p>Status: {status.join(', ')}</p>}
        {hosted === null ? null : <p data-slot="preview-host">{hosted}.</p>}
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
          <Button variant="outline" size="sm" className="mt-1 w-fit" onClick={() => store.dismiss()}>
            <X aria-hidden="true" />
            Close preview
          </Button>
        ) : null}
      </div>
    </div>
  )
}
