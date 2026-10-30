import { Ban, Check, CircleSlash, LogOut, Plus, Sparkles, Undo2, Zap } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { MARK_WORDS } from './marks'
import type { MarkFx } from './model'

const ICONS: Readonly<Record<MarkFx, LucideIcon>> = {
  activated: Zap,
  triggered: Sparkles,
  resolved: Check,
  countered: Ban,
  fizzled: CircleSlash,
  exiled: LogOut,
  bounced: Undo2,
  drawn: Plus,
}

/**
 * What a spell, ability or move did, as a badge on a card, a stack chip or a flying card: an icon and a short word in the
 * colour of the kind. Decoration only (the log says it too), so it is hidden from assistive technology; `fx.css`
 * colours it (centred on the card, or in line on a stack chip) and, where motion is welcome, fades it in and out within `--fx-mark-ms`.
 */
export function FxMark({ kind, inline = false }: { kind: MarkFx; inline?: boolean }) {
  const Icon = ICONS[kind]
  return (
    <span
      aria-hidden="true"
      data-slot="fx-mark"
      data-fx-mark={kind}
      data-inline={inline || undefined}
      className="fx-mark"
    >
      <Icon aria-hidden="true" className="size-3.5 shrink-0" />
      <span className="truncate">{MARK_WORDS[kind]}</span>
    </span>
  )
}
