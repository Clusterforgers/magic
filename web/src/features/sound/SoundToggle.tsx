import { Switch } from '@/components/ui/switch'
import {
  setSoundsOn,
  setSoundVolume,
  setTableSoundsOn,
  useSoundsOn,
  useSoundVolume,
  useTableSoundsOn,
} from './soundPrefs'
import { playCue } from './synth'

/**
 * The "Sounds" section of the settings page: the cues on or off, the table's sounds (casts, attacks, draws) on or off,
 * and one volume for both, which plays a cue as it is set. Kept in this browser.
 */
export function SoundToggle() {
  const on = useSoundsOn()
  const table = useTableSoundsOn()
  const volume = useSoundVolume()
  return (
    <section className="grid gap-3" aria-labelledby="sounds-heading">
      <h2 id="sounds-heading" className="text-lg font-semibold">
        Sounds
      </h2>
      <label className="flex items-center justify-between gap-4 text-sm">
        <span>
          Play a short, quiet sound when your turn starts, when a decision waits for you and when you win.
          Kept in this browser.
        </span>
        <Switch data-slot="sounds-toggle" checked={on} onCheckedChange={setSoundsOn} />
      </label>
      <label className="flex items-center justify-between gap-4 text-sm">
        <span>
          Also play the table: spells cast and resolving, attacks, blocks, damage, draws and shuffles.
        </span>
        <Switch
          data-slot="table-sounds-toggle"
          checked={on && table}
          disabled={!on}
          onCheckedChange={setTableSoundsOn}
        />
      </label>
      <label className="grid gap-1 text-sm">
        <span>Volume</span>
        <input
          type="range"
          data-slot="sounds-volume"
          min={0}
          max={100}
          step={5}
          value={Math.round(volume * 100)}
          disabled={!on}
          aria-valuetext={`${Math.round(volume * 100)} %`}
          className="w-full max-w-xs accent-primary"
          onChange={(event) => {
            const next = Number(event.target.value) / 100
            setSoundVolume(next)
            playCue('decision', next)
          }}
        />
      </label>
    </section>
  )
}
