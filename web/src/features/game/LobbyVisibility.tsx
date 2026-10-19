import { Globe, Lock } from 'lucide-react'
import { VISIBILITY_WORDS } from '@/lib/visibility'
import type { Visibility } from '@/protocol/generated'

const ICONS = { public: Globe, invite: Lock } as const

/** Who may find the game, in the words the create page promised; nothing until the lookup has said. */
export function LobbyVisibility({ visibility }: { visibility: Visibility | undefined }) {
  if (visibility === undefined) return null
  const Icon = ICONS[visibility]
  return (
    <p
      data-slot="lobby-visibility"
      data-visibility={visibility}
      className="mt-2 flex items-center gap-2 text-sm text-muted-foreground"
    >
      <Icon aria-hidden="true" className="size-4 shrink-0" />
      <span>{VISIBILITY_WORDS[visibility].status}</span>
    </p>
  )
}
