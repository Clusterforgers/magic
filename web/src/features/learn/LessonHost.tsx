import { useNavigate } from '@tanstack/react-router'
import { useCallback, useState } from 'react'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { Coach } from './Coach'
import { lessonOf, nextLesson } from './course'
import type { LessonSession } from './lessonSession'
import type { LessonId } from './step'
import { markDone, startLesson } from './tutorials'

/** What `LessonHost` takes. */
export interface LessonHostProps {
  lesson: LessonId | 'manual'
  game: string
  session: LessonSession
  api?: Pick<Api, 'startTutorial'>
}

/** The coach of a lesson's game, with its way out: restart, the next lesson, and back to the Learn page. */
export function LessonHost({ lesson: id, game, session, api = sharedApi }: LessonHostProps) {
  const navigate = useNavigate()
  const lesson = lessonOf(id)
  const next = nextLesson(id)
  const [busy, setBusy] = useState(false)
  const open = useCallback(
    async (to: LessonId | 'manual') => {
      if (busy) return
      setBusy(true)
      try {
        const start = await startLesson(api, to)
        // A new game is a new page: the session and the coach start fresh.
        window.location.assign(`/game/${start.game}#${start.hash}`)
      } catch {
        setBusy(false)
      }
    },
    [api, busy],
  )
  const finish = useCallback(() => markDone(id), [id])
  return (
    <Coach
      lesson={lesson}
      game={game}
      session={session}
      onRestart={() => void open(id)}
      onLeave={() => void navigate({ to: '/learn' })}
      onFinish={finish}
      onNextLesson={next === null ? undefined : () => void open(next.id)}
      nextTitle={next?.title}
    />
  )
}
