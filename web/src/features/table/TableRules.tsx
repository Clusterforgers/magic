import { ScrollTextIcon } from 'lucide-react'
import { useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { usePlay } from '@/features/game/gameContext'
import { describeRules } from '@/lib/houseRules'

/**
 * The house rules of this game that differ from its format's own, in the top bar: a button with the count that opens
 * the list. Nothing is shown when the game plays by its format's rules.
 */
export function TableRules() {
  const rules = usePlay((state) => state.view?.house_rules)
  const format = usePlay((state) => state.view?.format ?? 'normal')
  const words = useMemo(() => describeRules(rules, format), [rules, format])
  if (words.length === 0) return null
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            data-slot="table-rules"
            variant="ghost"
            size="sm"
            aria-label={`House rules: ${words.length} ${words.length === 1 ? 'differs' : 'differ'} from the format`}
          />
        }
      >
        <ScrollTextIcon aria-hidden="true" />
        <span aria-hidden="true" className="max-sm:sr-only">
          House rules
        </span>
        <span aria-hidden="true" className="rounded bg-foreground/15 px-1 text-xs leading-4">
          {words.length}
        </span>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-auto max-w-[calc(100vw-1rem)] gap-2 p-3">
        <PopoverTitle className="text-sm font-semibold">House rules</PopoverTitle>
        <ul aria-label="House rules of this game" className="grid gap-1 text-sm">
          {words.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  )
}
