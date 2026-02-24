import { useMemo, useRef, useState } from "react";

import {
  A2uiRenderer,
  createDemoDocument,
  useA2uiRuntime,
  type A2uiFormSubmitEvent,
  type PatchDocument
} from "./a2ui";

const LOAD_TEST_PATCH_COUNT = 220;

export function App() {
  const initialDocument = useMemo(() => createDemoDocument(), []);
  const patchSequenceRef = useRef(0);
  const [runtimeNote, setRuntimeNote] = useState<string>("Ready.");

  const runtime = useA2uiRuntime(initialDocument, {
    budget: {
      maxOpsPerTick: 64,
      maxQueueDepth: 512,
      maxApplyMsPerTick: 8
    },
    onBudgetYield: (result) => {
      setRuntimeNote(
        `Frame budget yield after ${result.appliedPatches} patches (${result.elapsedMs.toFixed(2)} ms).`
      );
    },
    onPatchError: (error) => {
      setRuntimeNote(`Patch queue rejected update: ${error.message}`);
    }
  });

  function handleFormSubmit(event: A2uiFormSubmitEvent): void {
    setRuntimeNote(
      `Form '${event.componentId}' submitted with ${Object.keys(event.values).length} fields.`
    );
  }

  function applySinglePatch(): void {
    patchSequenceRef.current += 1;
    const tick = patchSequenceRef.current;
    const patch: PatchDocument = {
      v: 1,
      ops: [
        {
          op: "replace",
          path: "/components/0/props/value",
          value: `Renderer online. Applied tick #${tick}.`
        },
        {
          op: "replace",
          path: "/components/3/props/rows/1/1",
          value: String(runtime.pendingPatchCount)
        },
        {
          op: "replace",
          path: "/components/5/props/series/2/value",
          value: 20 + (tick % 11)
        }
      ]
    };
    runtime.enqueuePatch(patch);
    setRuntimeNote(`Queued interactive patch #${tick}.`);
  }

  function applyLoadPatchBurst(): void {
    for (let index = 0; index < LOAD_TEST_PATCH_COUNT; index += 1) {
      patchSequenceRef.current += 1;
      const tick = patchSequenceRef.current;
      runtime.enqueuePatch({
        v: 1,
        ops: [
          {
            op: "replace",
            path: "/components/0/props/value",
            value: `Renderer online. Burst tick #${tick}.`
          },
          {
            op: "replace",
            path: "/components/5/props/series/0/value",
            value: 8 + (tick % 9)
          }
        ]
      });
    }
    setRuntimeNote(`Queued ${LOAD_TEST_PATCH_COUNT} patches for load simulation.`);
  }

  function resetDemoDocument(): void {
    runtime.replaceDocument(createDemoDocument());
    patchSequenceRef.current = 0;
    setRuntimeNote("Demo document reset.");
  }

  return (
    <div className="console-shell">
      <header className="console-shell__header">
        <div>
          <p className="console-shell__eyebrow">Palyra / M34</p>
          <h1>A2UI Renderer Web v1</h1>
          <p className="console-shell__subtitle">
            Safe React renderer with incremental patch updates and explicit frame budgets.
          </p>
        </div>
        <div className="console-shell__actions">
          <button type="button" onClick={applySinglePatch}>
            Queue Patch
          </button>
          <button type="button" onClick={applyLoadPatchBurst}>
            Load Burst
          </button>
          <button type="button" className="button--ghost" onClick={resetDemoDocument}>
            Reset
          </button>
        </div>
      </header>

      <section className="console-shell__status" aria-live="polite">
        <p>
          <strong>Pending queue:</strong> {runtime.pendingPatchCount}
        </p>
        <p>
          <strong>Runtime note:</strong> {runtimeNote}
        </p>
      </section>

      <main className="console-shell__main">
        <A2uiRenderer document={runtime.document} onFormSubmit={handleFormSubmit} />
      </main>

      <footer className="console-shell__footer">
        <p>
          Security baseline: No HTML injection, no payload-defined event handlers, markdown links limited to
          `http/https/mailto`.
        </p>
      </footer>
    </div>
  );
}
