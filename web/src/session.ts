import type { ColoringBounds } from './generated/ColoringBounds'
import type { ColoringKind } from './generated/ColoringKind'
import type { ColoringResult } from './generated/ColoringResult'
import type { PaletteSearch } from './generated/PaletteSearch'
import type { Problem } from './generated/Problem'
import type { ProblemText } from './generated/ProblemText'
import type { ZeroRoundResult } from './generated/ZeroRoundResult'
import type { DiagramOptions } from './generated/DiagramOptions'
import type { DiagramResult } from './generated/DiagramResult'
import type { Diagram } from './generated/Diagram'
import type { DiagramKind } from './generated/DiagramKind'
import type { MaximizationResult } from './generated/MaximizationResult'

export type DiagramLayout = 'hierarchical' | 'physics' | 'free'
export type SavedDiagram = {
  options: DiagramOptions
  layout: DiagramLayout
  results: DiagramResult[]
}

export function newSavedDiagram(): SavedDiagram {
  return { options: { kind: 'output_only', output: 'exact', input_output: 'cheap' }, layout: 'physics', results: [] }
}

type LegacyDiagramKind = 'output' | 'input_specific' | 'uniform'

/** Version-one files and links may predate the consistent diagram names. */
function restoreDiagramKind(kind?: DiagramKind | LegacyDiagramKind): DiagramKind {
  switch (kind) {
    case undefined:
    case 'output': return 'output_only'
    case 'input_specific': return 'input_output'
    case 'uniform': return 'input_aware'
    default: return kind
  }
}

function restoreDiagram(saved?: SavedDiagram): SavedDiagram {
  if (!saved) return newSavedDiagram()
  // Earlier version-one sessions stored a view selector and one three-diagram
  // result. Keep those already-computed relations in the per-choice cache.
  const older = saved as SavedDiagram & { view?: DiagramKind | LegacyDiagramKind; result?: DiagramResult & Partial<Record<LegacyDiagramKind, Diagram>> }
  const kind = restoreDiagramKind(saved.options.kind ?? older.view)
  const options = { ...saved.options, kind }
  let results = saved.results
  if (!results) {
    const previous = older.result
    results = !previous ? [] : previous.diagram ? [previous]
      : (['output', 'input_specific', 'uniform'] as const).map(kind => ({
        options: { ...previous.options, kind: restoreDiagramKind(kind) }, diagram: previous[kind]!,
      }))
  }
  return { options, layout: saved.layout ?? 'physics', results: results.map(result => ({
    ...result, options: { ...result.options, kind: restoreDiagramKind(result.options.kind) },
  })) }
}

/** Browser state only; problems and analysis results use the Rust protocol types. */
export type ColoringQuery = { kind: ColoringKind; search: PaletteSearch }
export type CachedColoring = ColoringQuery & { result: ColoringResult }
export type SavedColoring = {
  kind: ColoringKind
  search: 'maximum' | 'fixed'
  colors: number
  bounds: ColoringBounds | null
  results: CachedColoring[]
}

export function coloringQuery(selection: Pick<SavedColoring, 'kind' | 'search' | 'colors'>): ColoringQuery {
  return { kind: selection.kind, search: selection.search === 'fixed'
    ? { kind: 'fixed', colors: selection.colors } : { kind: 'maximum' } }
}

export function coloringKey(query: ColoringQuery): string {
  return query.search.kind === 'fixed' ? `${query.kind}:fixed:${query.search.colors}` : `${query.kind}:maximum`
}

function restoreColoring(saved: SavedColoring): SavedColoring {
  // Earlier version-one files stored only the selected result. Retain it as
  // the first cache entry, without leaving the obsolete field in new exports.
  const older = saved as SavedColoring & { result?: ColoringResult | null }
  const results = saved.results ?? (older.result ? [{ ...coloringQuery(saved), result: older.result }] : [])
  return { kind: saved.kind, search: saved.search, colors: saved.colors, bounds: saved.bounds, results }
}

export type ProblemSection = 'input' | 'output' | 'mapping' | 'zeroRound' | 'coloring' | 'diagram' | 'maximization'
  | 'inputActive' | 'inputPassive' | 'outputActive' | 'outputPassive'
export type CollapsedSections = Partial<Record<ProblemSection, boolean>>

export type SavedProblem = {
  problem: Problem
  collapsed: boolean
  collapsedSections: CollapsedSections
  zeroRound: ZeroRoundResult | null
  maximization: MaximizationResult | null
  coloring: SavedColoring
  diagram: SavedDiagram
}

export type Session = { editor: ProblemText; problems: SavedProblem[] }
type SessionFile = { format: 'round-eliminator-3'; version: 1; session: Session }

export function newSavedProblem(problem: Problem): SavedProblem {
  return {
    problem, collapsed: false, collapsedSections: {}, zeroRound: null, maximization: null,
    coloring: { kind: 'vertex', search: 'maximum', colors: 3, bounds: null, results: [] },
    diagram: newSavedDiagram(),
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
  // Version-one sessions written before diagram support have no diagram state.
  return { ...session, problems: session.problems.map(entry => {
    // Graph class is now always-visible header metadata; discard its old fold flag.
    const { graph: _graph, ...collapsedSections } = (entry.collapsedSections ?? {}) as CollapsedSections & { graph?: boolean }
    return { ...entry, collapsedSections, maximization: entry.maximization ?? null,
      coloring: restoreColoring(entry.coloring), diagram: restoreDiagram(entry.diagram) }
  }) }
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
