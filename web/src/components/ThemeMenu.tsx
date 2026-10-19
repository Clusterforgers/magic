import { MonitorIcon, MoonIcon, SunIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { parseChoice, useTheme } from '@/hooks/useTheme'
import type { ThemeChoice, ThemeStore } from '@/hooks/useTheme'

const OPTIONS: ReadonlyArray<{ value: ThemeChoice; label: string; Icon: LucideIcon }> = [
  { value: 'light', label: 'Light', Icon: SunIcon },
  { value: 'dark', label: 'Dark', Icon: MoonIcon },
  { value: 'system', label: 'System', Icon: MonitorIcon },
]

/** The theme control of the header: an icon button showing the choice in force, opening Light, Dark and System. */
export function ThemeMenu({ store }: { store?: ThemeStore }) {
  const { choice, setChoice } = useTheme(store)
  const current = OPTIONS.find((option) => option.value === choice) ?? OPTIONS[2]
  if (current === undefined) return null
  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger
          render={
            <DropdownMenuTrigger
              render={<Button variant="ghost" size="icon" aria-label={`Theme: ${current.label}`} />}
            />
          }
        >
          <current.Icon aria-hidden="true" />
        </TooltipTrigger>
        <TooltipContent>Theme</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup value={choice} onValueChange={(value) => setChoice(parseChoice(value))}>
          {OPTIONS.map(({ value, label, Icon }) => (
            <DropdownMenuRadioItem key={value} value={value} closeOnClick>
              <Icon aria-hidden="true" />
              {label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
