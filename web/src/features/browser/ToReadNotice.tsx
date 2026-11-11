import { Link } from '@tanstack/react-router'
import { BookOpenIcon, ChevronRightIcon } from 'lucide-react'
import { useAuth } from '@/features/auth/context'
import { useToReadCount } from '@/features/reads/useToReadCount'

/** "N cards wait for your read": the signed-in user's cards to read, as a link to Review cards; nothing for none. */
export function ToReadNotice() {
  const account = useAuth((state) => (state.status === 'signed-in' ? (state.user?.id ?? null) : null))
  const count = useToReadCount(account)
  if (count === 0) return null
  return (
    <Link
      to="/review"
      search={{ mine: true }}
      data-slot="to-read-notice"
      className="inline-flex w-fit items-center gap-2 rounded-lg border border-l-4 border-l-support-read bg-surface px-3 py-2 text-sm font-medium transition-colors duration-fast ease-out hover:bg-muted"
    >
      <BookOpenIcon aria-hidden="true" className="size-4 shrink-0 text-support-read" />
      {`${count} ${count === 1 ? 'card waits' : 'cards wait'} for your read`}
      <ChevronRightIcon aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
    </Link>
  )
}
