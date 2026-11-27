/**
 * What a reworked boss brings, on the intro of its fight: its two effects (the innate at its level and this fight's
 * modifier), whom it hunts, its two phases with the round each begins at, its Satchel with each card's mark, and its relics.
 * Every card carries its own rules text from the plan the game is made with.
 */
import type { ReactNode } from 'react'
import { BossAbilityList } from '@/features/table/campaign/BossAbilities'
import { reworkedAbilities } from '@/features/table/campaign/reworkedBoss'
import { SatchelMarks } from '@/features/table/campaign/SatchelMarks'
import type { DelveFightView } from '@/protocol/generated'

function Part({
  id,
  title,
  note,
  children,
}: {
  id: string
  title: string
  note?: string
  children: ReactNode
}) {
  return (
    <section aria-labelledby={id} data-slot={`intro-${id}`} className="grid gap-2">
      <h3 id={id} className="text-sm font-semibold">
        {title}
      </h3>
      {note === undefined ? null : <p className="text-xs text-muted-foreground">{note}</p>}
      {children}
    </section>
  )
}

/** The reworked boss's parts on the intro of the fight against `name`. */
export function ReworkedIntro({ fight, name }: { fight: DelveFightView; name: string }) {
  const abilities = reworkedAbilities(fight)
  const effects = abilities.filter((ability) => ability.kind === 'innate' || ability.kind === 'modifier')
  const phases = abilities.filter((ability) => ability.kind === 'next-phase' || ability.kind === 'phase')
  const relics = abilities.filter((ability) => ability.kind === 'relic')
  const hunts = abilities.filter((ability) => ability.kind === 'personality')
  const satchel = fight.satchel ?? []
  return (
    <>
      {effects.length === 0 ? null : (
        <Part id="effects-title" title={`What ${name} does`} note="Both are in play from the first turn.">
          <BossAbilityList abilities={effects} />
        </Part>
      )}
      {hunts.length === 0 ? null : (
        <Part
          id="personality-title"
          title={`Whom ${name} hunts`}
          note="It eyes this threat first, but never past the spotlight's cap: a hero who took more than their share is left alone."
        >
          <BossAbilityList abilities={hunts} />
        </Part>
      )}
      {phases.length === 0 ? null : (
        <Part
          id="phases-title"
          title="Phases"
          note="Each phase begins at the end of its round and stays for the rest of the fight."
        >
          <BossAbilityList abilities={phases} />
        </Part>
      )}
      {satchel.length === 0 ? null : (
        <Part
          id="satchel-title"
          title="Satchel"
          note={`${name} casts these from its command zone. A card that returns costs {2} more each time.`}
        >
          <SatchelMarks cards={satchel} className="sm:grid-cols-2" />
        </Part>
      )}
      {relics.length === 0 ? null : (
        <Part id="relics-title" title="Relics" note={`Relics from the heroes' pool, working for ${name}.`}>
          <BossAbilityList abilities={relics} />
        </Part>
      )}
    </>
  )
}
