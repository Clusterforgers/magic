import { Switch } from '@/components/ui/switch'
import { setSoundsOn, useSoundsOn } from './soundPrefs'

/** The "Sounds" switch of the settings page: turns the table's short cues on or off in this browser. */
export function SoundToggle() {
  const on = useSoundsOn()
  return (
    <section className="grid gap-2" aria-labelledby="sounds-heading">
      <h2 id="sounds-heading" className="text-lg font-semibold">
        Sounds
      </h2>
      <label className="flex items-center justify-between gap-4 text-sm">
        <span>
          Play a short, quiet sound when your turn starts, when a decision waits for you and when you win. Kept in this
          browser.
        </span>
        <Switch data-slot="sounds-toggle" checked={on} onCheckedChange={setSoundsOn} />
      </label>
    </section>
  )
}
