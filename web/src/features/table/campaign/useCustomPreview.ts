/** The preview handlers of a control that stands for a boss ability: it shows that ability's custom card. */
import { useEffect } from 'react'
import type { FocusEvent, MouseEvent, PointerEvent } from 'react'
import { anchorOf, usePreviewStore } from '../preview'
import type { BossAbility } from './bossAbilities'
import { abilityShown, customPreviewId } from './customCards'
import { KIND_WORDS, useTableContent } from './useBossAbilities'

/** The events and the object id mark of a control whose preview is a custom card; spread them on its element. */
export interface CustomPreviewHandlers {
  'data-object-id': number
  onClick(event: MouseEvent<HTMLElement>): void
  onPointerEnter(event: PointerEvent<HTMLElement>): void
  onPointerLeave(): void
  onFocus(event: FocusEvent<HTMLElement>): void
  onBlur(): void
}

/**
 * Hover (mouse only) and focus show the custom card of `ability` of the boss `owner` in the card preview, leaving
 * hides it, a press pins it (a second press closes it); null without an ability. `all` are the boss's abilities, whose
 * phases a phase's preview lists.
 */
export function useCustomPreview(
  ability: BossAbility | undefined,
  all: readonly BossAbility[],
  owner: string,
): CustomPreviewHandlers | null {
  const preview = usePreviewStore()
  const content = useTableContent()
  const id = customPreviewId(ability?.card ?? '')
  // A control that leaves the screen while hovered gets no pointer-leave.
  useEffect(() => () => preview.hide(id), [preview, id])
  if (ability === undefined) return null
  const shown = () => abilityShown(ability, all, content, `In ${owner}'s command zone`, KIND_WORDS)
  return {
    'data-object-id': id,
    onClick: (event) => preview.pin(id, anchorOf(event.currentTarget), undefined, shown()),
    onPointerEnter(event) {
      if (event.pointerType === 'mouse') preview.show(id, anchorOf(event.currentTarget), undefined, shown())
    },
    onPointerLeave: () => preview.hide(id),
    onFocus: (event) => preview.show(id, anchorOf(event.currentTarget), undefined, shown()),
    onBlur: () => preview.hide(id),
  }
}
