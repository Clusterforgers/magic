import { ChevronDown, ChevronUp, ChevronsLeft, ChevronsRight } from 'lucide-react'
import { useId, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { ChatPanel } from './ChatPanel'
import { LogPanel } from './LogPanel'

type DrawerTab = 'log' | 'chat'

// A side column fits from the `lg` breakpoint; below it the sheet starts folded to its tab bar.
function startsOpen(): boolean {
  return typeof window.matchMedia !== 'function' || window.matchMedia('(min-width: 1024px)').matches
}

/**
 * The game log and the chat. A collapsible column on the right on wide screens; on narrow ones a tab bar at the
 * bottom whose tabs open a sheet above it.
 */
export function Drawer() {
  const [open, setOpen] = useState(startsOpen)
  const [tab, setTab] = useState<DrawerTab>('log')
  const bodyId = useId()
  return (
    <aside
      aria-label="Game log and chat"
      data-open={open}
      className={cn(
        'fixed inset-x-0 bottom-0 z-30 border-t bg-background',
        open ? 'h-[55dvh]' : 'h-auto',
        'lg:sticky lg:top-0 lg:z-auto lg:h-[calc(100dvh-3rem)] lg:shrink-0 lg:self-start lg:border-t-0 lg:border-l',
        open ? 'lg:w-80' : 'lg:w-12',
      )}
    >
      <Tabs
        value={tab}
        onValueChange={(value) => {
          if (value === 'log' || value === 'chat') setTab(value)
        }}
        className="h-full gap-0"
      >
        <div className="flex items-center justify-between gap-1 border-b p-1.5">
          <TabsList className={cn(!open && 'lg:hidden')}>
            <TabsTrigger value="log" onClick={() => setOpen(true)}>
              Log
            </TabsTrigger>
            <TabsTrigger value="chat" onClick={() => setOpen(true)}>
              Chat
            </TabsTrigger>
          </TabsList>
          <Button
            variant="ghost"
            size="icon"
            aria-expanded={open}
            aria-controls={bodyId}
            aria-label={open ? 'Collapse log and chat' : 'Expand log and chat'}
            onClick={() => setOpen((was) => !was)}
          >
            {open ? (
              <ChevronDown aria-hidden="true" className="lg:hidden" />
            ) : (
              <ChevronUp aria-hidden="true" className="lg:hidden" />
            )}
            {open ? (
              <ChevronsRight aria-hidden="true" className="max-lg:hidden" />
            ) : (
              <ChevronsLeft aria-hidden="true" className="max-lg:hidden" />
            )}
          </Button>
        </div>
        <div id={bodyId} className={cn('min-h-0 flex-1 p-2', !open && 'hidden')}>
          {open ? (
            <>
              <TabsContent value="log" className="h-full">
                <LogPanel />
              </TabsContent>
              <TabsContent value="chat" className="h-full">
                <ChatPanel />
              </TabsContent>
            </>
          ) : null}
        </div>
      </Tabs>
    </aside>
  )
}
