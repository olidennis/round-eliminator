import type { CachedColoring } from './generated/CachedColoring'
import type { ColoringBounds } from './generated/ColoringBounds'
import type { ColoringKind } from './generated/ColoringKind'
import type { ColoringResult } from './generated/ColoringResult'
import type { PaletteSearch } from './generated/PaletteSearch'
import type { ProblemState } from './generated/ProblemState'
import type { AnalysisCache } from './generated/AnalysisCache'
import type { Problem } from './generated/Problem'
import type { ProblemText } from './generated/ProblemText'
import type { ZeroRoundResult } from './generated/ZeroRoundResult'
import type { DiagramOptions } from './generated/DiagramOptions'
import type { DiagramResult } from './generated/DiagramResult'
import type { Diagram } from './generated/Diagram'
import type { DiagramKind } from './generated/DiagramKind'
import type { MaximizationResult } from './generated/MaximizationResult'

export type DiagramLayout = 'hierarchical' | 'physics' | 'free'
export type SavedDiagram = { options: DiagramOptions; layout: DiagramLayout }
type LegacyDiagramOptions = Omit<DiagramOptions, 'output'> & { output: DiagramOptions['output'] | 'maximized' }
type LegacyDiagramResult = Omit<DiagramResult, 'options'> & { options: LegacyDiagramOptions }
type LegacySavedDiagram = Omit<SavedDiagram, 'options'> & { options: LegacyDiagramOptions; results?: LegacyDiagramResult[] }

/** The retired assumption-based method becomes an exact request. Its result
 * cannot certify completeness, so recompute it rather than reuse the assumption. */
function restoreDiagramOptions(options: LegacyDiagramOptions): DiagramOptions {
  return { ...options, kind: restoreDiagramKind(options.kind),
    output: options.output === 'maximized' ? 'exact' : options.output }
}
function restoreDiagramResults(results: LegacyDiagramResult[]): DiagramResult[] {
  return results.filter(result => result.options.output !== 'maximized')
    .map(result => ({ ...result, options: restoreDiagramOptions(result.options) }))
}

export function newSavedDiagram(): SavedDiagram {
  return { options: { kind: 'output_only', output: 'exact', input_output: 'cheap' }, layout: 'physics' }
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

function restoreDiagram(saved?: LegacySavedDiagram): SavedDiagram & { results: DiagramResult[] } {
  if (!saved) return { ...newSavedDiagram(), results: [] }
  // Earlier version-one sessions stored a view selector and one three-diagram
  // result. Keep those already-computed relations in the per-choice cache.
  const older = saved as LegacySavedDiagram & { view?: DiagramKind | LegacyDiagramKind; result?: DiagramResult & Partial<Record<LegacyDiagramKind, Diagram>> }
  const kind = restoreDiagramKind(saved.options.kind ?? older.view)
  const options = restoreDiagramOptions({ ...saved.options, kind })
  let results = saved.results
  if (!results) {
    const previous = older.result
    results = !previous ? [] : previous.diagram ? [previous]
      : (['output', 'input_specific', 'uniform'] as const).map(kind => ({
        options: { ...previous.options, kind: restoreDiagramKind(kind) }, diagram: previous[kind]!,
      }))
  }
  return { options, layout: saved.layout ?? 'physics', results: restoreDiagramResults(results) }
}

/** Browser state only; problems and analysis results use the Rust protocol types. */
export type ColoringQuery = { kind: ColoringKind; search: PaletteSearch }
export type { CachedColoring } from './generated/CachedColoring'
export type SavedColoring = {
  kind: ColoringKind
  search: 'maximum' | 'fixed'
  colors: number
}

export function coloringQuery(selection: Pick<SavedColoring, 'kind' | 'search' | 'colors'>): ColoringQuery {
  return { kind: selection.kind, search: selection.search === 'fixed'
    ? { kind: 'fixed', colors: selection.colors } : { kind: 'maximum' } }
}

export function coloringKey(query: ColoringQuery): string {
  return query.search.kind === 'fixed' ? `${query.kind}:fixed:${query.search.colors}` : `${query.kind}:maximum`
}

function restoreColoring(saved: SavedColoring & { bounds?: ColoringBounds | null; results?: CachedColoring[] }): SavedColoring & { bounds: ColoringBounds | null; results: CachedColoring[] } {
  // Earlier version-one files stored only the selected result. Retain it as
  // the first cache entry, without leaving the obsolete field in new exports.
  const older = saved as SavedColoring & { result?: ColoringResult | null }
  const results = saved.results ?? (older.result ? [{ ...coloringQuery(saved), result: older.result }] : [])
  return { kind: saved.kind, search: saved.search, colors: saved.colors, bounds: saved.bounds ?? null, results }
}

export type ProblemSection = 'input' | 'output' | 'mapping' | 'zeroRound' | 'coloring' | 'diagram' | 'maximization'
  | 'inputActive' | 'inputPassive' | 'outputActive' | 'outputPassive'
export type CollapsedSections = Partial<Record<ProblemSection, boolean>>

export type SavedProblem = {
  state: ProblemState
  collapsed: boolean
  collapsedSections: CollapsedSections
  coloring: SavedColoring
  diagram: SavedDiagram
}

export type Session = { editor: ProblemText; problems: SavedProblem[] }
type SessionFile = { format: 'round-eliminator-3'; version: 2; session: Session }

export function newSavedProblem(problem: Problem): SavedProblem {
  return {
    state: { problem, analyses: { maximization: null, zero_round: null, coloring: [], diagrams: [], coloring_bounds: [] } }, collapsed: false, collapsedSections: {},
    coloring: { kind: 'vertex', search: 'maximum', colors: 3 },
    diagram: { options: newSavedDiagram().options, layout: 'physics' },
  }
}

export function serializeSession(session: Session): string {
  // Save proofs, not an in-flight query. Imported computations never resume.
  const file: SessionFile = {
    format: 'round-eliminator-3', version: 2,
    session,

  }
  return JSON.stringify(file, null, 2)
}

export function deserializeSession(text: string): Session {
  const file = JSON.parse(text)
  if (file?.format !== 'round-eliminator-3') {
    throw new Error('This is not a Round Eliminator 3 session. Old Round Eliminator files are not supported.')
  }
  if (file.version !== 1 && file.version !== 2) throw new Error('Unsupported Round Eliminator 3 session version.')
  const session = file.session
  if (!session?.editor || !['plain', 'independent', 'mapped', 'paired'].includes(session.editor.kind)
    || !session.editor.degrees || !Array.isArray(session.problems)) {
    throw new Error('The session is missing its editor or problem data.')
  }
  return { ...session, problems: session.problems.map((entry: SavedProblem & {
    problem?: Problem; zeroRound?: ZeroRoundResult | null; maximization?: MaximizationResult | null;
  }) => {
    const problem = entry.state?.problem ?? entry.problem
    if (!problem?.graph_class || !problem.input || !problem.output || !problem.mapping || !entry.coloring) {
      throw new Error('The session is missing its editor or problem data.')
    }
    // Older snapshots stored caches beside their UI controls. Move those proofs
    // into the shared computational state, keeping view settings independently.
    const oldColoring = restoreColoring(entry.coloring)
    const oldDiagram = restoreDiagram(entry.diagram as LegacySavedDiagram)
    const restored: AnalysisCache = entry.state?.analyses ?? {
      maximization: entry.maximization ?? null, zero_round: entry.zeroRound ?? null,
      coloring: oldColoring.results, diagrams: oldDiagram.results, coloring_bounds: [],
    }
    const coloringBounds = (restored.coloring_bounds ?? []).map(entry => ({ ...entry, bounds: { ...entry.bounds, testing: null } }))
    if (entry.state?.analyses?.coloring_bounds === undefined && oldColoring.bounds && (oldColoring.bounds.lower !== null || oldColoring.bounds.upper !== null)
      && !coloringBounds.some(entry => entry.kind === oldColoring.kind)) {
      coloringBounds.push({ kind: oldColoring.kind, bounds: { ...oldColoring.bounds, testing: null } })
    }
    const analyses = { ...restored, coloring_bounds: coloringBounds, diagrams: restoreDiagramResults(restored.diagrams) }
    const { graph: _graph, ...collapsedSections } = (entry.collapsedSections ?? {}) as CollapsedSections & { graph?: boolean }
    return { state: { problem, analyses }, collapsed: entry.collapsed ?? false, collapsedSections,
      coloring: { kind: oldColoring.kind, search: oldColoring.search, colors: oldColoring.colors },
      diagram: { options: oldDiagram.options, layout: oldDiagram.layout },
    }
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
