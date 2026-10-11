import type { ColoringBounds } from './generated/ColoringBounds'
import type { ColoringKind } from './generated/ColoringKind'
import type { ColoringResult } from './generated/ColoringResult'
import type { Problem } from './generated/Problem'
import type { ProblemText } from './generated/ProblemText'
import type { ZeroRoundResult } from './generated/ZeroRoundResult'

/** Browser state only; problems and analysis results use the Rust protocol types. */
export type SavedColoring = {
  kind: ColoringKind
  search: 'maximum' | 'fixed'
  colors: number
  bounds: ColoringBounds | null
  result: ColoringResult | null
}

export type ProblemSection = 'graph' | 'input' | 'output' | 'mapping' | 'zeroRound' | 'coloring'
  | 'inputActive' | 'inputPassive' | 'outputActive' | 'outputPassive'
export type CollapsedSections = Partial<Record<ProblemSection, boolean>>

export type SavedProblem = {
  problem: Problem
  collapsed: boolean
  collapsedSections: CollapsedSections
  zeroRound: ZeroRoundResult | null
  coloring: SavedColoring
}

export type Session = { editor: ProblemText; problems: SavedProblem[] }
type SessionFile = { format: 'round-eliminator-3'; version: 1; session: Session }

export function newSavedProblem(problem: Problem): SavedProblem {
  return {
    problem, collapsed: false, collapsedSections: {}, zeroRound: null,
    coloring: { kind: 'vertex', search: 'maximum', colors: 3, bounds: null, result: null },
  }
}

export function serializeSession(session: Session): string {
  // Save proofs, not an in-flight query. Imported computations never resume.
  const file: SessionFile = {
    format: 'round-eliminator-3', version: 1,
    session: { ...session, problems: session.problems.map(entry => ({
      ...entry, coloring: { ...entry.coloring,
        bounds: entry.coloring.bounds ? { ...entry.coloring.bounds, testing: null } : null,
      },
    })) },
  }
  return JSON.stringify(file, null, 2)
}

export function deserializeSession(text: string): Session {
  const file = JSON.parse(text) as SessionFile
  if (file?.format !== 'round-eliminator-3') {
    throw new Error('This is not a Round Eliminator 3 session. Old Round Eliminator files are not supported.')
  }
  if (file.version !== 1) throw new Error('Unsupported Round Eliminator 3 session version.')
  const session = file.session
  if (!session?.editor || !['plain', 'independent', 'mapped', 'paired'].includes(session.editor.kind)
    || !session.editor.degrees || !Array.isArray(session.problems)
    || !session.problems.every(entry => entry?.problem?.graph_class && entry.problem.input
      && entry.problem.output && entry.problem.mapping && entry.coloring && 'zeroRound' in entry)) {
    throw new Error('The session is missing its editor or problem data.')
  }
  // As with analysis requests, snapshots from this version are trusted. Do not
  // reparse problem notation or repeat Rust's semantic construction checks.
  return session
}

/** Compressed UTF-8 in the fragment keeps shared sessions out of HTTP requests. */
export async function sessionLink(session: Session): Promise<string> {
  const stream = new Blob([serializeSession(session)]).stream()
    .pipeThrough(new CompressionStream('gzip'))
  const bytes = new Uint8Array(await new Response(stream).arrayBuffer())
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  const url = new URL(location.href)
  url.hash = 're3=' + btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '')
  return url.href
}

export async function sessionFromHash(hash: string): Promise<Session> {
  if (!hash.startsWith('#re3=')) throw new Error('This is not a Round Eliminator 3 session link.')
  const binary = atob(hash.slice(5).replaceAll('-', '+').replaceAll('_', '/'))
  const bytes = Uint8Array.from(binary, char => char.charCodeAt(0))
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'))
  return deserializeSession(await new Response(stream).text())
}
