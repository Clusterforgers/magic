/** `/delves`: the player's Delves runs, a new run (Heat and an optional seed code) and joining a friend's by code. */
import { Link, useNavigate } from '@tanstack/react-router'
import { CircleAlertIcon, SkullIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Pill } from '@/components/Pill'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Skeleton } from '@/components/ui/skeleton'
import { SignedInOnly } from '@/features/tournaments/SignedInOnly'
import { usePageTitle } from '@/hooks/usePageTitle'
import { delvesApi } from '@/net/delves'
import type { DelvesApi } from '@/net/delves'
import type { DelveSummary } from '@/protocol/generated'
import { errorText, statusText } from './words'

function NewDelveForm({ client }: { client: DelvesApi }) {
  const navigate = useNavigate()
  const [heat, setHeat] = useState('0')
  const [seed, setSeed] = useState('')
  const [code, setCode] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const open = (id: string) => navigate({ to: '/delves/$delveId', params: { delveId: id } })
  const run = (task: Promise<{ id: string }>) => {
    setBusy(true)
    setError(null)
    task
      .then((view) => open(view.id))
      .catch((caught: unknown) => {
        setBusy(false)
        setError(errorText(caught))
      })
  }
  const create = (event: FormEvent) => {
    event.preventDefault()
    const trimmed = seed.trim()
    run(client.create({ heat: Number(heat), ...(trimmed === '' ? {} : { seed: trimmed }) }))
  }
  const join = (event: FormEvent) => {
    event.preventDefault()
    if (code.trim() === '') {
      setError('Enter the invite code your friend shared.')
      return
    }
    run(client.join(code))
  }
  return (
    <div className="grid gap-4 sm:grid-cols-2">
      <form onSubmit={create} aria-label="New delve" className="surface grid gap-3 p-4" noValidate>
        <h2 className="text-lg font-semibold">New delve</h2>
        <div className="grid gap-1">
          <Label htmlFor="delve-heat">Heat</Label>
          <NativeSelect id="delve-heat" value={heat} onChange={(e) => setHeat(e.target.value)}>
            <NativeSelectOption value="0">Heat 0: the bosses as written</NativeSelectOption>
            <NativeSelectOption value="1">Heat 1: bosses have a fifth more life</NativeSelectOption>
            <NativeSelectOption value="2">Heat 2: and play one level smarter</NativeSelectOption>
          </NativeSelect>
        </div>
        <div className="grid gap-1">
          <Label htmlFor="delve-seed">Seed code (optional)</Label>
          <Input
            id="delve-seed"
            value={seed}
            onChange={(e) => setSeed(e.target.value)}
            placeholder="Random"
          />
        </div>
        <Button type="submit" disabled={busy}>
          Create delve
        </Button>
      </form>
      <form
        onSubmit={join}
        aria-label="Join a delve"
        className="surface grid content-start gap-3 p-4"
        noValidate
      >
        <h2 className="text-lg font-semibold">Join a friend</h2>
        <div className="grid gap-1">
          <Label htmlFor="delve-code">Invite code</Label>
          <Input id="delve-code" value={code} onChange={(e) => setCode(e.target.value)} />
        </div>
        <Button type="submit" variant="outline" disabled={busy}>
          Join
        </Button>
      </form>
      {error !== null ? (
        <p role="alert" className="text-sm text-destructive sm:col-span-2">
          {error}
        </p>
      ) : null}
    </div>
  )
}

function DelveRow({ delve }: { delve: DelveSummary }) {
  return (
    <li className="flex flex-wrap items-center justify-between gap-2 border-b p-3 last:border-b-0">
      <div className="grid gap-0.5">
        <Link
          to="/delves/$delveId"
          params={{ delveId: delve.id }}
          className="font-medium underline-offset-4 hover:underline"
        >
          {delve.theme}
        </Link>
        <span className="text-sm text-muted-foreground">
          {delve.heroes.join(', ')} · {delve.cleared} of {delve.bosses} bosses
        </span>
      </div>
      <Pill>{statusText(delve.status)}</Pill>
    </li>
  )
}

function DelveList({ client }: { client: DelvesApi }) {
  const [list, setList] = useState<DelveSummary[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let live = true
    client
      .list()
      .then((answer) => live && setList(answer.delves))
      .catch((caught: unknown) => live && setError(errorText(caught)))
    return () => {
      live = false
    }
  }, [client])
  if (error !== null) {
    return (
      <Alert variant="destructive">
        <CircleAlertIcon aria-hidden="true" />
        <AlertTitle>Could not load your delves</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    )
  }
  if (list === null) {
    return (
      <Loading label="Loading your delves…">
        <Skeleton className="h-24" />
      </Loading>
    )
  }
  if (list.length === 0) {
    return (
      <EmptyState icon={SkullIcon} title="No delves yet">
        Gather up to five friends and bot allies, bring your Commander decks and fight through a ladder of
        bosses.
      </EmptyState>
    )
  }
  return (
    <ul aria-label="Your delves" className="surface">
      {list.map((delve) => (
        <DelveRow key={delve.id} delve={delve} />
      ))}
    </ul>
  )
}

export function DelvesPage({ client = delvesApi }: { client?: DelvesApi }) {
  usePageTitle('Delves')
  return (
    <PageShell width="page" className="grid gap-6 sm:py-6">
      <div className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">Delves</h1>
        <p className="text-sm text-muted-foreground">
          A co-op boss rush: your party fights the Warrens' bosses one after another, life carried between
          fights, loot between bosses.
        </p>
      </div>
      <SignedInOnly next="/delves">
        <DelveList client={client} />
        <NewDelveForm client={client} />
      </SignedInOnly>
    </PageShell>
  )
}
