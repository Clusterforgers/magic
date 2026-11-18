import { useContext, useId } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { DECISION_LABEL } from '../../turnText'
import type { DecisionName } from '../../turnText'
import { useSide } from '../../sideContext'
import { StackRest } from '../../StackPanel'
import { ChoiceOverlayContext } from '../choiceOverlay'
import { ownDecision } from '../ownDecision'
import { hasDetail, kindKey, sheetTitle, startsFolded } from './asking'
import { DecisionDetail } from './DecisionDetail'
import { useAsker } from './useAsker'

/**
 * The decision's detail where no side column holds it (a phone, a rail, a folded column): a sheet over the hand with a
 * grab handle that folds it to one line. It starts folded for a decision answered on the table, and is folded while the
 * centre overlay lays out a prompt's cards, whose band lies just above the hand.
 */
export function DecisionSheet({ phone }: { phone: boolean }) {
  const side = useSide()
  const pending = usePlay(ownDecision)
  const overlay = useContext(ChoiceOverlayContext)
  const asker = useAsker()
  const body = useId()
  if (side.kind === 'column' || pending === null || !hasDetail(pending)) return null
  const forced = overlay?.open === true
  const folded = forced || (side.sheetFolded ?? startsFolded(pending))
  const kind = kindKey(pending.decision.kind)
  const label = Object.hasOwn(DECISION_LABEL, kind) ? DECISION_LABEL[kind as DecisionName] : kind
  return (
    <div
      data-slot="decision-sheet-cell"
      // The hand's row: the sheet stands on the window's bottom edge and grows up over the hand and your board.
      style={{ gridRow: 5, gridColumn: phone ? '1 / -1' : 1 }}
      className="pointer-events-none relative z-35 min-h-0 min-w-0"
    >
      <section
        aria-label="Decision detail"
        data-slot="decision-sheet"
        data-folded={folded || undefined}
        className="pointer-events-auto decision-sheet"
      >
        <button
          type="button"
          data-slot="sheet-grab"
          aria-expanded={!folded}
          aria-controls={body}
          aria-label={folded ? 'Open the stack sheet' : 'Fold the stack sheet'}
          disabled={forced}
          onClick={() => side.setSheetFolded(!folded)}
          className="grid min-h-11 w-full flex-none justify-items-center gap-1 px-3 pt-1.5 pb-0.5 outline-offset-[-2px] outline-primary focus-visible:outline-2"
        >
          <i aria-hidden="true" className="h-1 w-10 rounded-full bg-border-strong" />
          <span className="max-w-full truncate text-xs leading-4 font-semibold text-muted-foreground">
            Stack · {sheetTitle(asker, label, kind === 'PayMana')}
          </span>
        </button>
        <div
          id={body}
          hidden={folded}
          className="min-h-0 overflow-y-auto overscroll-contain px-2.5 pb-3 [scrollbar-width:thin]"
        >
          <DecisionDetail where="sheet">
            <StackRest />
          </DecisionDetail>
        </div>
      </section>
    </div>
  )
}
