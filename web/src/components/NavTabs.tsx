import { Link, useLocation } from '@tanstack/react-router'
import {
  CastleIcon,
  ChartNoAxesColumnIcon,
  ChevronDownIcon,
  HistoryIcon,
  LayersIcon,
  ListChecksIcon,
  PackageIcon,
  SwordsIcon,
  TrophyIcon,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'
import type { ComponentPropsWithRef } from 'react'
import { isCurrentTab, tabsThatFit } from '@/components/navFit'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLinkItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useToReadCount } from '@/features/reads/useToReadCount'
import { TO_READ } from '@/lib/toRead'
import { cn } from '@/lib/utils'

// A tab of the main navigation: quiet until it is the current page, which gets a bar under it as well as strong text.
const TAB =
  "relative inline-flex h-11 shrink-0 items-center border-b-2 whitespace-nowrap border-transparent px-2 text-sm font-medium sm:px-3 text-muted-foreground transition-colors duration-fast ease-out hover:text-foreground aria-[current='page']:border-primary aria-[current='page']:text-foreground data-current:border-primary data-current:text-foreground sm:h-14"

const BADGE =
  'inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-support-read px-1 text-xs font-semibold text-on-primary tabular-nums'

/** The addresses of the tabs. */
type TabPath = '/' | '/precons' | '/decks' | '/games' | '/tournaments' | '/delves' | '/review' | '/support'

/** A tab of the main navigation. */
interface NavTab {
  to: TabPath
  label: string
  /** Its picture in the More menu. */
  icon: LucideIcon
  /** Current only at its own address, not under it. */
  exact?: boolean
}

const PLAY: NavTab = { to: '/', label: 'Play', icon: SwordsIcon, exact: true }
const PRECONS: NavTab = { to: '/precons', label: 'Precons', icon: PackageIcon }

/** The tabs of a guest. */
const GUEST_TABS: readonly NavTab[] = [PLAY, PRECONS]

/** The tabs of an account, most used first: the row keeps the first that fit and the More menu takes the rest. */
const ACCOUNT_TABS: readonly NavTab[] = [
  PLAY,
  PRECONS,
  { to: '/decks', label: 'Decks', icon: LayersIcon },
  { to: '/games', label: 'My games', icon: HistoryIcon },
  { to: '/tournaments', label: 'Tournaments', icon: TrophyIcon },
  { to: '/delves', label: 'Delves', icon: CastleIcon },
  { to: '/review', label: 'Review cards', icon: ListChecksIcon },
  { to: '/support', label: 'Card support', icon: ChartNoAxesColumnIcon },
]

const badgeOf = (count: number): string => (count > 99 ? '99+' : String(count))

/** A tab's words and, on Review cards while cards wait, the count to read. */
function TabWords({ tab, toRead }: { tab: NavTab; toRead: number }) {
  return (
    <>
      <span>{tab.label}</span>
      {tab.to === '/review' && toRead > 0 ? (
        <span aria-hidden="true" data-slot="to-read-badge" className={BADGE}>
          {badgeOf(toRead)}
        </span>
      ) : null}
    </>
  )
}

/** The link of a tab, in the row or in the More menu; Review cards opens the review of the account's decks. */
function TabLink({
  tab,
  toRead,
  checking,
  ...rest
}: {
  tab: NavTab
  toRead: number
  /** On the cards to check, which live under /decks but are not Decks. */
  checking: boolean
} & Omit<ComponentPropsWithRef<'a'>, 'href'>) {
  const label = tab.to === '/review' && toRead > 0 ? `${tab.label}, ${toRead} ${TO_READ}` : undefined
  const common = { 'aria-label': label, ...rest }
  if (tab.to === '/review')
    return <Link to="/review" search={{ mine: true }} data-slot="review-cards" {...common} />
  const exact = tab.exact === true || (tab.to === '/decks' && checking)
  return <Link to={tab.to} activeOptions={{ exact }} {...common} />
}

/**
 * The tabs of the main navigation in the room the brand and the buttons leave. Those that fit stand in the row; the
 * rest go into a More menu at its end, which is marked when the current page is in it and counts the cards to read
 * when Review cards is. A hidden copy of the row measures every tab, so the split follows the window, the fonts and
 * the badge without the row ever being cut off or scrolling.
 */
export function NavTabs({ account }: { account: string | null }) {
  const tabs = account === null ? GUEST_TABS : ACCOUNT_TABS
  const toRead = useToReadCount(account)
  const pathname = useLocation({ select: (location) => location.pathname })
  const checking = pathname === '/decks/unvalidated'
  const row = useRef<HTMLDivElement>(null)
  const ruler = useRef<HTMLDivElement>(null)
  const [shown, setShown] = useState(tabs.length)

  useLayoutEffect(() => {
    const box = row.current
    const parts = ruler.current
    if (box === null || parts === null) return
    const fit = (): void => {
      const widths = Array.from(
        parts.querySelectorAll('[data-measure="tab"]'),
        (part) => part.getBoundingClientRect().width,
      )
      const more = parts.querySelector('[data-measure="more"]')?.getBoundingClientRect().width ?? 0
      setShown(tabsThatFit(widths, more, box.clientWidth))
    }
    fit()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(fit)
    observer.observe(box)
    for (const part of Array.from(parts.children)) observer.observe(part)
    return () => observer.disconnect()
  }, [tabs, toRead])

  const visible = tabs.slice(0, shown)
  const hidden = tabs.slice(shown)
  const current = hidden.some((tab) =>
    isCurrentTab(tab.to, pathname, tab.exact === true || (tab.to === '/decks' && checking)),
  )
  const hiddenToRead = hidden.some((tab) => tab.to === '/review') ? toRead : 0
  const moreLabel = [
    'More pages',
    current ? 'the current page is one of them' : null,
    hiddenToRead > 0 ? `${hiddenToRead} ${hiddenToRead === 1 ? 'card' : 'cards'} ${TO_READ}` : null,
  ]
    .filter((part) => part !== null)
    .join(', ')

  return (
    <div ref={row} data-slot="nav-tabs" className="relative flex min-w-0 flex-1 items-center">
      {visible.map((tab) => (
        <TabLink key={tab.to} tab={tab} toRead={toRead} checking={checking} className={cn(TAB, 'gap-2')}>
          <TabWords tab={tab} toRead={toRead} />
        </TabLink>
      ))}
      {hidden.length > 0 ? (
        <DropdownMenu>
          <DropdownMenuTrigger
            data-slot="nav-more"
            data-current={current || undefined}
            aria-label={moreLabel}
            className={cn(
              TAB,
              'cursor-pointer gap-1 outline-none focus-visible:ring-2 focus-visible:ring-ring',
            )}
          >
            More
            {hiddenToRead > 0 ? (
              <span aria-hidden="true" className={BADGE}>
                {badgeOf(hiddenToRead)}
              </span>
            ) : null}
            <ChevronDownIcon aria-hidden="true" className="size-4" />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="min-w-52">
            {hidden.map((tab) => (
              <DropdownMenuLinkItem
                key={tab.to}
                closeOnClick
                render={(props) => <TabLink {...props} tab={tab} toRead={toRead} checking={checking} />}
                className="aria-[current='page']:font-semibold aria-[current='page']:text-foreground"
              >
                <tab.icon aria-hidden="true" className="size-4 text-muted-foreground" />
                <span className="flex-1">{tab.label}</span>
                {tab.to === '/review' && toRead > 0 ? (
                  <span aria-hidden="true" className={BADGE}>
                    {badgeOf(toRead)}
                  </span>
                ) : null}
              </DropdownMenuLinkItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      ) : null}
      {/* The ruler: every tab and the More button as drawn, unseen and out of reach, for the widths of the split. */}
      <div
        ref={ruler}
        aria-hidden="true"
        inert
        className="pointer-events-none invisible absolute inset-x-0 top-0 flex h-0 overflow-hidden"
      >
        {tabs.map((tab) => (
          <span key={tab.to} data-measure="tab" className={cn(TAB, 'gap-2')}>
            <TabWords tab={tab} toRead={toRead} />
          </span>
        ))}
        <span data-measure="more" className={cn(TAB, 'gap-1')}>
          More
          {toRead > 0 ? <span className={BADGE}>{badgeOf(toRead)}</span> : null}
          <ChevronDownIcon className="size-4" />
        </span>
      </div>
    </div>
  )
}
