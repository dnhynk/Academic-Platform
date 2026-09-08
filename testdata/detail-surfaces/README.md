# Synthetic detail-surface inputs

`corpus.json` and `synthetic-lecture.wav` are invented inputs for the explicit,
feature-gated detail fixture seeder. They contain no personal records or real
recording. The WAV has twelve seconds of deterministic PCM tone and silence;
it proves neither realistic lecture playback nor speech-to-text alignment.

Build and verify the exact inputs with:

```powershell
pnpm --filter @academic-os/ui build
node tools/detail-fixture.mjs
node tools/detail-fixture.mjs --check
```

The builder owns both files. The desktop asset bundle contains neither the
corpus nor the WAV. Native detail views read the host-selected synthetic profile;
the runtime never uses these files as a fallback for missing profile evidence.

The corpus includes every named detail section, an intentionally unmapped
transcript segment, retained non-speech/redacted/failure dispositions, reported
relation states, question revisions and a stale repository snapshot. Imported
labels do not establish user confirmation or domain-engine computation.

Ordinary domain producers, full original recording support, policy-staged egress
preview, A5 acceptance and real-data admission remain separate and incomplete.
