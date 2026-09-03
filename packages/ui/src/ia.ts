/**
 * Reads the global information architecture out of the specification itself.
 *
 * `route_manifest_matches_ia_exactly` needs both sides of the comparison to be
 * enumerated from an independent source. The route manifest is written by hand;
 * this reads section 25.1's tree out of
 * `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and returns it as a
 * list of labelled nodes with their parents.
 *
 * The parser is deliberately strict. A line it cannot account for raises rather
 * than being skipped, because a skipped line is a destination that silently
 * stops being required.
 */

import { readFile } from "node:fs/promises";

/** One line of the section 25.1 tree. */
export interface IaNode {
  /** The label exactly as the specification spells it. */
  readonly label: string;
  /** Label of the parent node, or `null` for the root. */
  readonly parentLabel: string | null;
  /** Zero for the root, one for a section, two for a leaf. */
  readonly depth: number;
}

/** The heading that introduces the tree. */
const IA_HEADING = "### 25.1 전역 IA";

/** One level of indentation in the specification's tree drawing. */
const INDENT_UNITS: readonly string[] = ["│  ", "   "];

/** The two branch markers the specification's tree drawing uses. */
const BRANCH_MARKERS: readonly string[] = ["├─ ", "└─ "];

/**
 * The fenced block that follows the section 25.1 heading.
 *
 * The heading is matched on its own line and the fence is the first one after
 * it, so a later section that happens to contain a tree cannot be read instead.
 */
export function extractIaFence(specification: string): string {
  const lines = specification.split(/\r?\n/u);
  const headingIndex = lines.indexOf(IA_HEADING);
  if (headingIndex < 0) {
    throw new Error(`the specification has no ${IA_HEADING} heading`);
  }
  let openIndex = -1;
  for (let index = headingIndex + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line === undefined) {
      continue;
    }
    if (line.startsWith("### ") || line.startsWith("## ")) {
      break;
    }
    if (line.startsWith("```")) {
      openIndex = index;
      break;
    }
  }
  if (openIndex < 0) {
    throw new Error(`${IA_HEADING} is not followed by a fenced block`);
  }
  const closeIndex = lines.indexOf("```", openIndex + 1);
  if (closeIndex < 0) {
    throw new Error(`the ${IA_HEADING} fence is not closed`);
  }
  return lines.slice(openIndex + 1, closeIndex).join("\n");
}

/** Splits one drawn line into its indentation depth and its label. */
function parseTreeLine(line: string): { depth: number; label: string } {
  let rest = line;
  let depth = 0;
  for (;;) {
    const unit = INDENT_UNITS.find((candidate) => rest.startsWith(candidate));
    if (unit === undefined) {
      break;
    }
    rest = rest.slice(unit.length);
    depth += 1;
  }
  const marker = BRANCH_MARKERS.find((candidate) => rest.startsWith(candidate));
  if (marker === undefined) {
    if (depth > 0) {
      throw new Error(`indented tree line carries no branch marker: ${JSON.stringify(line)}`);
    }
    return { depth: 0, label: rest };
  }
  return { depth: depth + 1, label: rest.slice(marker.length) };
}

/**
 * Parses the drawn tree into labelled nodes.
 *
 * Every non-empty line must parse. A line whose depth jumps by more than one
 * has no parent to attach to and raises.
 */
export function parseIaTree(fence: string): readonly IaNode[] {
  const nodes: IaNode[] = [];
  const parentByDepth = new Map<number, string>();
  for (const raw of fence.split("\n")) {
    const line = raw.replace(/\s+$/u, "");
    if (line.length === 0) {
      continue;
    }
    const { depth, label } = parseTreeLine(line);
    if (label.length === 0) {
      throw new Error(`tree line has an empty label: ${JSON.stringify(raw)}`);
    }
    if (depth === 0) {
      if (nodes.length > 0) {
        throw new Error(`the tree has a second root: ${JSON.stringify(label)}`);
      }
      nodes.push({ label, parentLabel: null, depth });
    } else {
      const parentLabel = parentByDepth.get(depth - 1);
      if (parentLabel === undefined) {
        throw new Error(`tree line has no parent at depth ${String(depth)}: ${JSON.stringify(label)}`);
      }
      nodes.push({ label, parentLabel, depth });
    }
    parentByDepth.set(depth, label);
    for (const known of [...parentByDepth.keys()]) {
      if (known > depth) {
        parentByDepth.delete(known);
      }
    }
  }
  if (nodes.length === 0) {
    throw new Error("the section 25.1 fence parsed to no nodes at all");
  }
  return nodes;
}

/** Reads and parses section 25.1 from a specification file. */
export async function readIaTree(specificationPath: URL | string): Promise<readonly IaNode[]> {
  return parseIaTree(extractIaFence(await readFile(specificationPath, "utf8")));
}
