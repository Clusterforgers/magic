import { ListChecks, X } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { Switch } from '@/components/ui/switch'
import { useGame, usePlay } from '@/features/game/gameContext'
import { AUTO_SKIP_DESCRIPTION, autoSkipListName, ruleKey, ruleWords } from '../autoSkips'
import { canSetStops } from '../selectors'

/**
 * "Auto-skip triggers" beside Full Control: a switch that asks the server to pass for the seat at every triggered
 * ability on top of the stack (`AutoChoices.skip_triggers`, kept with the account like the other auto choices), and one
 * button that opens the per-card list: the triggers it auto-passes and its rules for its own triggers, each with a
 * remove button. Both show what the server holds; Full Control sets them all aside while on.
 */
export function AutoSkipToggle() {
  const { session } = useGame()
  const { shown, auto, online, paused, yields, skips } = usePlay(
    useShallow((state) => ({
      shown: canSetStops(state),
      auto: state.autoChoices,
      online: state.status === 'open',
      paused: state.fullControl !== 'Off',
      yields: state.autoYields,
      skips: state.autoSkips,
    })),
  )
  if (!shown) return null
  const on = auto.skip_triggers === true
  const count = yields.length + skips.length
  return (
    <div
      role="group"
      aria-label="Auto-skip triggers"
      data-slot="auto-skip"
      className="inline-flex items-center gap-1"
    >
      <label className="flex h-8 items-center gap-1.5 px-1 text-xs font-medium">
        <Switch
          data-slot="auto-skip-switch"
          checked={on}
          disabled={!online}
          aria-label="Auto-skip triggers"
          aria-description={
            paused ? `${AUTO_SKIP_DESCRIPTION} Off while Full Control is on.` : AUTO_SKIP_DESCRIPTION
          }
          onCheckedChange={(checked) => session.setAutoChoices({ ...auto, skip_triggers: checked })}
        />
        <span aria-hidden="true" className={paused ? 'opacity-60' : undefined}>
          Auto-skip triggers
        </span>
      </label>
      <Popover>
        <PopoverTrigger
          render={
            <Button
              data-slot="auto-skip-list-button"
              variant="outline"
              size="icon-sm"
              aria-label={autoSkipListName(count)}
            />
          }
        >
          <ListChecks aria-hidden="true" />
        </PopoverTrigger>
        <PopoverContent side="top" align="end" className="w-72 max-w-[calc(100vw-1rem)] gap-1 p-2 text-sm">
          <PopoverTitle className="text-sm font-semibold">
            Auto-skips
            {paused ? (
              <span className="font-normal text-muted-foreground"> (off in Full Control)</span>
            ) : null}
          </PopoverTitle>
          {count === 0 ? (
            <p className="text-muted-foreground">
              No per-card rules. Use "Auto-pass this trigger" on a trigger on the stack, or "Always" on a
              question of your own trigger.
            </p>
          ) : (
            <ul data-slot="auto-skip-list" className="grid gap-1">
              {yields.map((ability) => (
                <ListRow
                  key={`yield\n${ability.name}\n${ability.text}`}
                  label={`Auto-pass ${ability.name}`}
                  detail={ability.text}
                  remove={`Stop auto-passing ${ability.name}${ability.text === '' ? '' : `: ${ability.text}`}`}
                  disabled={!online}
                  onRemove={() => session.clearAutoYield(ability)}
                />
              ))}
              {skips.map((rule) => {
                const words = ruleWords(rule)
                return (
                  <ListRow
                    key={ruleKey(rule)}
                    label={words.label}
                    detail=""
                    remove={words.remove}
                    disabled={!online}
                    onRemove={() => session.cancelAutoSkip(rule)}
                  />
                )
              })}
            </ul>
          )}
        </PopoverContent>
      </Popover>
    </div>
  )
}

function ListRow(props: {
  label: string
  detail: string
  remove: string
  disabled: boolean
  onRemove: () => void
}) {
  return (
    <li className="flex items-center gap-2">
      <span className="min-w-0 flex-1 truncate" title={props.detail === '' ? undefined : props.detail}>
        {props.label}
      </span>
      <Button
        size="icon-sm"
        variant="ghost"
        data-slot="auto-skip-remove"
        aria-label={props.remove}
        disabled={props.disabled}
        onClick={props.onRemove}
      >
        <X aria-hidden="true" />
      </Button>
    </li>
  )
}
