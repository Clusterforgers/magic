import { CheckIcon, HandIcon } from 'lucide-react'
import { useContext } from 'react'
import { useCardDetail } from '@/features/builder/useCardDetail'
import { CardsApiContext } from '@/features/table/cardsContext'
import { RUNS_AUTOMATICALLY, YOU_DO_BY_HAND, lineMarks } from '@/lib/lineMarks'
import { cn } from '@/lib/utils'
import type { CardDetail } from '@/protocol/generated'
import type { PreviewCard } from './model'

function MarkTag({ hand }: { hand: boolean }) {
  const Icon = hand ? HandIcon : CheckIcon
  return (
    <span
      className={cn(
        'inline-flex shrink-0 items-center gap-1 rounded-sm border px-1 text-xs font-medium',
        hand ? 'border-support-manual text-support-manual' : 'border-border text-muted-foreground',
      )}
    >
      <Icon aria-hidden="true" className="size-3" />
      {hand ? YOU_DO_BY_HAND : RUNS_AUTOMATICALLY}
    </span>
  )
}

/** Each line of a partly automatic card's text with its mark, words and icon, so colour never carries it alone. */
export function LineMarksList({ detail }: { detail: CardDetail }) {
  const { faces, oracle_text: whole, reasons } = detail
  const texts = faces.length > 0 ? faces.map((face) => face.oracle_text) : [whole]
  return (
    <div
      role="group"
      aria-label="Lines of the card"
      data-slot="line-marks"
      className="grid gap-1 text-left text-sm"
    >
      {texts.flatMap((text, face) => {
        const { lines, unplaced } = lineMarks(text, reasons, face)
        return [
          ...lines.map((line, at) => (
            <p key={`${face}:${at}`} data-mark={line.mark ?? 'none'} className="flex items-start gap-2">
              {line.mark === null ? null : <MarkTag hand={line.mark === 'hand'} />}
              <span className="min-w-0 [overflow-wrap:anywhere]">{line.text}</span>
            </p>
          )),
          ...unplaced.map((reason) => (
            <p key={`${face}:u:${reason}`} data-mark="hand" className="flex items-start gap-2">
              <MarkTag hand />
              <span className="min-w-0 [overflow-wrap:anywhere]">{reason}</span>
            </p>
          )),
        ]
      })}
    </div>
  )
}

/** `LineMarksList` for a card of the deck preview, which loads the card detail itself. */
export function LineMarks({ card }: { card: PreviewCard }) {
  const { state } = useCardDetail(useContext(CardsApiContext), card.oracleId)
  return state.status === 'ready' ? <LineMarksList detail={state.detail} /> : null
}
