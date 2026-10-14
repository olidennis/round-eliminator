import type { Diagram } from './generated/Diagram'
import type { DiagramKind } from './generated/DiagramKind'
import type { InputOutputLabel } from './generated/InputOutputLabel'
import type { DiagramOptions } from './generated/DiagramOptions'
import type { DiagramResult } from './generated/DiagramResult'
import type { Problem } from './generated/Problem'

/** Pair labels come from the canonical name tables, including converted inputs. */
export function diagramNodeName(problem: Problem, node: InputOutputLabel): string {
  const output = problem.output.labels[node.output]
  const name = (label: string) => /[,()\s]/u.test(label) ? JSON.stringify(label) : label
  return node.input === null ? output : `(${name(problem.input.kind === 'explicit'
    ? problem.input.data.labels[node.input] : 'Trivial input')}, ${name(output)})`
}

/** Stable component IDs make exports unambiguous even with spaces in names. */
export function diagramText(problem: Problem, diagram: Diagram): string {
  const nodes = diagram.components.map((members, index) => `n${index} = `
    + members.map(member => JSON.stringify(diagramNodeName(problem, diagram.nodes[member]))).join(' '))
  const edges = diagram.edges.map(([source, target]) => `n${source} -> n${target}`)
  return `# ${diagram.complete ? 'Complete' : 'Partial: missing arrows are unknown'} passive diagram\n# nodes\n`
    + nodes.join('\n') + '\n# edges\n' + edges.join('\n') + '\n'
}
/** Ignore settings which do not affect the requested relation. */
export function diagramKey(options: DiagramOptions): string {
  if (options.kind === 'output_only') return `output_only:${options.output}`
  return options.input_output === 'exact'
    ? `${options.kind}:exact`
    : `${options.kind}:cheap:${options.output}`
}

/** Hide input relations only when the input promise makes them redundant. */
export function diagramKinds(problem: Problem): DiagramKind[] {
  if (problem.mapping.kind === 'restricted') return ['output_only', 'input_output', 'input_aware']
  if (problem.input.kind === 'trivial') return ['output_only']
  // Replacement has no context at degree zero or at degrees with no outputs.
  const relevant = problem.graph_class.passive.filter(degree => degree > 0
    && problem.output.passive.degrees.some(group => group.degree === degree && group.configurations.length > 0))
  const labelsByDegree = new Map(problem.input.data.passive.degrees.map(group => [group.degree,
    new Set(group.configurations.flatMap(configuration => configuration.parts.flatMap(part => part.labels))),
  ]))
  const kinds: DiagramKind[] = ['output_only']
  // Without mapping restrictions, the only input-specific information affecting
  // replacement is which degrees can contain that input label.
  if (problem.input.data.labels.some((_, input) => relevant.some(degree => !labelsByDegree.get(degree)?.has(input)))) {
    kinds.push('input_output')
  }
  // Input-aware replacement is output-only unless the promise excludes an entire
  // degree still present in the graph class and passive output constraint.
  if (relevant.some(degree => !labelsByDegree.get(degree)?.size)) kinds.push('input_aware')
  return kinds
}

/** Reuse an equivalent retired choice without running comparison or reduction. */
export function reuseOutputOnlyDiagram(problem: Problem, result: DiagramResult): DiagramResult | null {
  if (diagramKinds(problem).includes(result.options.kind)) return result
  const method = result.options.input_output === 'exact' ? 'exact' : result.options.output
  let diagram = result.diagram
  if (result.options.kind === 'input_output') {
    // Independent input diagrams have one output-only copy per input. Select
    // one copy and remap its existing node/component indices and reduced edges.
    const input = diagram.nodes[0]?.input
    const indices = diagram.nodes.flatMap((node, index) => node.input === input ? [index] : [])
    if (indices.length !== problem.output.labels.length) return null
    const nodes = new Map(indices.map((index, position) => [index, position]))
    const components = diagram.components.flatMap((members, index) => nodes.has(members[0]) ? [index] : [])
    const groups = new Map(components.map((index, position) => [index, position]))
    diagram = {
      nodes: indices.map(index => ({ input: null, output: diagram.nodes[index].output })),
      successors: indices.map(index => diagram.successors[index].map(target => nodes.get(target)!)),
      components: components.map(index => diagram.components[index].map(member => nodes.get(member)!)),
      edges: diagram.edges.filter(([source, target]) => groups.has(source) && groups.has(target))
        .map(([source, target]) => [groups.get(source)!, groups.get(target)!]),
      complete: diagram.complete,
    }
  }
  return { options: { ...result.options, kind: 'output_only', output: method },
    diagram: { ...diagram, complete: method !== 'partial' } }
}

