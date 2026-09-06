/** One outstanding native request owns the status and receipt it will publish. */
export type RuntimeOperation = "diagnostics" | "synthetic_ingest";
export interface RuntimeView {
  readonly pending: RuntimeOperation | null;
  readonly serviceMessage: string;
  readonly saveMessage: string;
  readonly receiptId: string | null;
}
export type NativeInvoke = (command: string, args: unknown) => Promise<unknown>;

export function runtimeRequests(invoke: NativeInvoke, render: (view: RuntimeView) => void): (command: RuntimeOperation) => Promise<void> {
  let view: RuntimeView = { pending: null, serviceMessage: "Connecting to the local service…", saveMessage: "No save requested.", receiptId: null };
  function publish(change: Partial<RuntimeView>): void { view = { ...view, ...change }; render(view); }
  return async (command) => {
    // Guard programmatic calls as well as disabled buttons. Diagnostics cannot
    // complete out of order and release another request's save action.
    if (view.pending !== null) return;
    const diagnostic = command === "diagnostics";
    publish(diagnostic
      ? { pending: command, serviceMessage: "Checking the local service…" }
      : { pending: command, saveMessage: "Saving synthetic example… Save is not yet confirmed.", receiptId: null });
    try {
      const reply = await invoke("desktop_request_v1", { request: { version: 1, operation: { command } } });
      if (typeof reply !== "object" || reply === null || !("version" in reply) || reply.version !== 1 || !("message" in reply) || typeof reply.message !== "string" || !("state" in reply) || typeof reply.state !== "string") throw new Error("Invalid native reply");
      if (reply.state === "accepted") {
        if (diagnostic || !("receipt_id" in reply) || !Array.isArray(reply.receipt_id) || reply.receipt_id.length !== 16 || !reply.receipt_id.every((value: unknown) => typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 255)) throw new Error("Missing immutable save receipt");
        publish({ saveMessage: "Synthetic example saved.", receiptId: (reply.receipt_id as number[]).map((value) => value.toString(16).padStart(2, "0")).join("") });
      } else {
        if (!["ready", "unavailable", "locked", "incompatible", "unsupported", "rejected"].includes(reply.state)) throw new Error("Unknown native state");
        const serviceMessage = reply.state === "unavailable" ? "Local service unavailable. Start it and check again."
          : reply.state === "ready" ? "Local service connected and unlocked. Synthetic data only." : reply.message;
        publish(diagnostic ? { serviceMessage } : { saveMessage: reply.message });
      }
    } catch {
      publish(diagnostic
        ? { serviceMessage: "The local service is unavailable or incompatible. Check the service before trying again." }
        : { saveMessage: "The local service could not confirm this save. Check the service before trying again.", receiptId: null });
    } finally { publish({ pending: null }); }
  };
}
