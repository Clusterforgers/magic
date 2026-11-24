/**
 * The notice that a descent led the party into another theme ("The party descends into the Drowned Spire"), in the new
 * theme's look, on the new floor until its first room is cleared.
 */
import { ArrowDownIcon } from 'lucide-react'
import type { DelveView } from '@/protocol/generated'
import { descentNotice } from './model'

/** The descent's notice, or nothing. */
export function DescentNotice({ view }: { view: DelveView }) {
  const words = descentNotice(view)
  if (words === null) return null
  return (
    <p
      role="status"
      data-slot="descent-notice"
      className="flex items-center gap-2 rounded-xl border-2 border-delve bg-delve-stone px-4 py-3 text-sm font-semibold shadow-e1 motion-safe:animate-in motion-safe:fade-in motion-safe:slide-in-from-top-2 motion-safe:duration-slow"
    >
      <ArrowDownIcon aria-hidden="true" className="size-5 shrink-0 text-delve" />
      {words}
    </p>
  )
}
