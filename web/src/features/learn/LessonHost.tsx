import { useNavigate } from '@tanstack/react-router'
import { useCallback, useState } from 'react'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { Coach } from './Coach'
import { lessonOf, nextLesson } from './course'
import type { LessonSession } from './lessonSession'
import type { LessonId } from './step'
import { leaveLesson, markDone, startLesson } from './tutorials'

/** What `LessonHost` takes. */
export interface LessonHostProps {
  lesson: LessonId | 'manual'
  game: string
  /** The learner's seat token, which ends the game when the learner leaves; undefined without one. */
  token: string | undefined
  session: LessonSession
  api?: Pick<Api, 'startTutorial' | 'endTutorial'>
}

/**
 * The coach of a lesson's game, with its way out: restart, the next lesson, and back to the Learn page. Each way out
 * ends this game: a new lesson replaces it, and leaving ends it.
 */
export function LessonHost({ lesson: id, game, token, session, api = sharedApi }: LessonHostProps) {
  const navigate = useNavigate()
  const lesson = lessonOf(id)
  const next = nextLesson(id)
  const [busy, setBusy] = useState(false)
  const open = useCallback(
    async (to: LessonId | 'manual') => {
      if (busy) return
      setBusy(true)
      // This game ends with the new one's start; its socket closes first, so its end reads as nothing here.
      session.disconnect()
      try {
        const start = await startLesson(api, to)
        // A new game is a new page: the session and the coach start fresh.
        window.location.assign(`/game/${start.game}#${start.hash}`)
      } catch {
        session.connect()
        setBusy(false)
      }
    },
    [api, busy, session],
  )
  const leave = useCallback(() => {
    session.disconnect()
    if (token !== undefined) void leaveLesson(api, { game, token })
    void navigate({ to: '/learn' })
  }, [api, game, navigate, session, token])
  const finish = useCallback(() => markDone(id), [id])
  return (
    <Coach
      lesson={lesson}
      game={game}
      session={session}
      onRestart={() => void open(id)}
      onLeave={leave}
      onFinish={finish}
      onNextLesson={next === null ? undefined : () => void open(next.id)}
      nextTitle={next?.title}
    />
  )
}
