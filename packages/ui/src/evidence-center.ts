/**
 * The content `P2-X1` left the `Evidence & Settings` branch framed for.
 *
 * `P2-X1` fixed the frame: every route in the section 25.1 tree has a titled
 * view with a breadcrumb, at least one section, and the evidence drawer, and
 * each section names the task that fills it. This file is `P2-X7` filling the
 * four it named.
 *
 * # There is one model, and it is not here
 *
 * `academic-evidence-center` owns what a centre entry is. This file is the
 * shell's side: which of section 25.1's four `Evidence & Settings` children
 * shows which of section 25.13's six sections, and nothing else. It holds no
 * entry, no digest, no identifier and no record — a section here is a heading
 * and a pointer, and `evidence-center.test.ts` compares the six against the
 * crate's own `CenterSection` enumeration read out of its source, so a section
 * renamed in Rust fails here rather than drifting silently.
 *
 * # No window opens
 *
 * No Tauri runtime is linked. What this file adds is that opening one of the
 * four destinations yields sections that name section 25.13's own content
 * rather than a promise that a later task will supply some. It is not evidence
 * that anything renders.
 */

/** One of section 25.13's six sections, as the shell shows it. */
export interface CenterSectionDefinition {
  /**
   * The crate's `CenterSection` arm, in `SCREAMING_SNAKE_CASE`.
   *
   * Compared against `academic_evidence_center::CenterSection` by
   * `the_shell_sections_are_the_crates_own`.
   */
  readonly id: string;
  /** The heading the shell renders. */
  readonly heading: string;
  /**
   * Section 25.13's own words for this section.
   *
   * The same string the crate's `CenterSection::spec_words` returns, and
   * compared against it.
   */
  readonly specWords: string;
  /** Which `Evidence & Settings` child route shows it. */
  readonly routeId: string;
}

/**
 * The six sections, in section 25.13's reading order.
 *
 * The `routeId` mapping is not one-to-one and is not meant to be: section 25.1
 * gives `Evidence & Settings` four children and section 25.13 names six
 * sections, so `Source / Claim Review` carries four of them. The mapping is
 * written out rather than derived, so a section that stopped having a home
 * fails as a route nobody shows.
 */
export const EVIDENCE_CENTER_SECTIONS: readonly CenterSectionDefinition[] = [
  {
    id: "PROPOSAL_INBOX",
    heading: "AI proposal inbox",
    specWords: "AI 제안 inbox",
    routeId: "evidence.source-claim-review",
  },
  {
    id: "OFFICIAL_SOURCE_CHANGE",
    heading: "Official source changes",
    specWords: "official source change",
    routeId: "evidence.source-claim-review",
  },
  {
    id: "UNRESOLVED_CONFLICT",
    heading: "Unresolved conflicts",
    specWords: "unresolved conflict",
    routeId: "evidence.source-claim-review",
  },
  {
    id: "LOW_CONFIDENCE",
    heading: "Low-confidence review queue",
    specWords: "low-confidence transcript/math/code",
    routeId: "evidence.source-claim-review",
  },
  {
    id: "PERMISSION_EXPIRY",
    heading: "Permission and consent expiry",
    specWords: "permission/consent expiry",
    routeId: "evidence.permissions-consent",
  },
  {
    id: "TRANSMISSION_LOG",
    heading: "Provider transmission log and deletion receipts",
    specWords: "provider transmission log와 deletion receipt",
    routeId: "evidence.privacy-providers",
  },
];

/** The route ids this file supplies content for. */
export const EVIDENCE_CENTER_ROUTES: readonly string[] = [
  "evidence",
  "evidence.source-claim-review",
  "evidence.permissions-consent",
  "evidence.privacy-providers",
];

/** Exactly the sections one route shows. */
export function sectionsForRoute(routeId: string): readonly CenterSectionDefinition[] {
  return EVIDENCE_CENTER_SECTIONS.filter((section) => section.routeId === routeId);
}
