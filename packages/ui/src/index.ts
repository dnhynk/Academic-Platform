/**
 * The desktop shell: route manifest, command palette, backlinks, evidence
 * drawer, optimistic-update typing, and the committed Tauri capability/CSP
 * snapshot's rules.
 *
 * `docs/contracts/desktop-shell.md` states what this is and is not evidence
 * for. In short: no Tauri runtime is linked and no window opens; what is fixed
 * here is the route manifest, the destinations it yields, the reachability of
 * the four entity types from every one of them, and the content of the
 * capability snapshot an audit can now diff.
 */

export * from "./backlinks.js";
export * from "./capability-snapshot.js";
export * from "./destinations.js";
export * from "./drawer.js";
export * from "./entities.js";
export * from "./evidence-center.js";
export * from "./ia.js";
export * from "./optimistic.js";
export * from "./palette.js";
export * from "./routes.js";
export * from "./shell.js";
export * from "./views.js";
