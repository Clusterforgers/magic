import { Link } from '@tanstack/react-router'
import { buttonVariants } from '@/components/ui/button'
import { usePageTitle } from '@/hooks/usePageTitle'

/** Shown when the address has no readable seat and token: nothing is connected until the player opens their link. */
export function SeatLinkMissing() {
  usePageTitle('Open your seat link')
  return (
    <div className="mx-auto grid w-full max-w-2xl gap-4 px-4 py-8 sm:px-6">
      <h1 className="text-2xl font-semibold tracking-tight">Open your seat link</h1>
      <p className="text-muted-foreground">
        This page needs the link made for your seat. It ends in something like{' '}
        <code className="rounded bg-muted px-1 py-0.5 text-sm">#seat=1&amp;token=…</code>, and the host gets
        it when the game is created.
      </p>
      <p className="text-muted-foreground">
        Open that link, or ask the host to send it again. If you did open a seat link, it may have been cut
        short when it was copied.
      </p>
      <div>
        <Link to="/" className={buttonVariants()}>
          Create a new game
        </Link>
      </div>
    </div>
  )
}
