/** Bounded verified WAV playback. No path, URL, blob fallback or autoplay. */
import type { DetailClient } from "./detail-client.js";
import { timecode } from "./details.js";

export interface LecturePlayer { seek(milliseconds: number): void; dispose(): void }
export function lecturePlayer(parent: HTMLElement, client: DetailClient, lectureId: string): LecturePlayer {
  const abort = new AbortController();
  const context = new AudioContext();
  const gain = context.createGain(); gain.connect(context.destination);
  let buffer: AudioBuffer | null = null;
  let source: AudioBufferSourceNode | null = null;
  let offset = 0; let started = 0; let frame = 0; let disposed = false;
  let starting = false;
  const live = (): boolean => !disposed;
  const status = document.createElement("p"); status.setAttribute("role", "status"); status.textContent = "Loading and verifying original audio from the selected profile…";
  const play = document.createElement("button"); play.type = "button"; play.textContent = "Play original audio"; play.disabled = true;
  const seek = document.createElement("input"); seek.type = "range"; seek.min = "0"; seek.max = "0"; seek.step = "0.01"; seek.value = "0"; seek.disabled = true; seek.setAttribute("aria-label", "Original audio position");
  const time = document.createElement("output"); time.textContent = "00:00.000";
  const volume = document.createElement("input"); volume.type = "range"; volume.min = "0"; volume.max = "1"; volume.step = "0.05"; volume.value = "1"; volume.setAttribute("aria-label", "Original audio volume");
  const waveform = document.createElementNS("http://www.w3.org/2000/svg", "svg"); waveform.setAttribute("viewBox", "0 0 640 80"); waveform.setAttribute("role", "img"); waveform.setAttribute("aria-label", "Waveform derived from the verified original recording"); waveform.classList.add("audio-waveform");
  parent.append(status, waveform, play, seek, time, volume);
  const current = (): number => source && buffer ? Math.min(buffer.duration, offset + context.currentTime - started) : offset;
  function update(): void {
    const position = current(); seek.value = String(position); time.textContent = timecode(Math.round(position * 1000)); seek.setAttribute("aria-valuetext", time.textContent);
    play.textContent = source ? "Pause original audio" : "Play original audio";
    if (source && !disposed) frame = requestAnimationFrame(update);
  }
  function pause(): void { offset = current(); const active = source; source = null; if (active) { active.onended = null; active.stop(); active.disconnect(); } cancelAnimationFrame(frame); if (!disposed) update(); }
  async function start(): Promise<void> {
    if (!buffer || disposed || starting) return;
    starting = true; play.disabled = true;
    try { await context.resume(); } finally { starting = false; if (live()) play.disabled = false; }
    if (abort.signal.aborted) return;
    if (offset >= buffer.duration) offset = 0;
    const active = context.createBufferSource(); active.buffer = buffer; active.connect(gain);
    source = active; started = context.currentTime;
    active.onended = () => { active.disconnect(); if (source === active) { source = null; offset = buffer?.duration ?? 0; cancelAnimationFrame(frame); if (!disposed) update(); } };
    active.start(0, offset); update();
  }
  const changePosition = (seconds: number): void => {
    if (disposed || !Number.isFinite(seconds) || seconds < 0) return;
    if (buffer && seconds > buffer.duration) { status.textContent = "This timestamp lies outside the available original audio. The raw segment remains visible."; return; }
    const playing = source !== null; pause(); offset = seconds; update(); if (playing) void start().catch(() => { if (!disposed) status.textContent = "Playback unavailable. Try playing the verified audio again."; });
  };
  play.addEventListener("click", () => { if (source) pause(); else void start().catch(() => { if (!disposed) status.textContent = "Playback unavailable. Try playing the verified audio again."; }); });
  seek.addEventListener("input", () => { changePosition(Number(seek.value)); });
  volume.addEventListener("input", () => { if (!disposed) gain.gain.value = Number(volume.value); });
  void client.audio(lectureId, abort.signal).then(async (audio) => {
    if (abort.signal.aborted) return;
    const decoded = await context.decodeAudioData(Uint8Array.from(audio.bytes).buffer);
    if (disposed) return;
    buffer = decoded; seek.max = String(decoded.duration); seek.disabled = false; play.disabled = false;
    parent.dataset.audioDigest = audio.digest; parent.dataset.audioLecture = audio.lectureId; parent.dataset.audioDuration = String(decoded.duration);
    status.textContent = `Verified original WAV · ${timecode(Math.round(decoded.duration * 1000))} · bounded reader (up to 4 MiB).`;
    if (offset > decoded.duration) { offset = 0; status.textContent += " The selected timestamp is outside this recording; the raw segment remains visible."; }
    const samples = decoded.getChannelData(0); const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    const points: string[] = [];
    for (let bin = 0; bin < 160; bin++) {
      let peak = 0; const end = Math.floor((bin + 1) * samples.length / 160);
      for (let at = Math.floor(bin * samples.length / 160); at < end; at++) peak = Math.max(peak, Math.abs(samples[at] ?? 0));
      points.push(`M${String(bin * 4 + 2)} ${String(40 - peak * 38)}V${String(40 + peak * 38)}`);
    }
    path.setAttribute("d", points.join(" ")); waveform.append(path); update();
  }).catch((error: unknown) => { if (!disposed) { status.textContent = String(error); play.disabled = true; seek.disabled = true; } });
  return {
    seek: (milliseconds) => { changePosition(milliseconds / 1000); },
    dispose: () => { if (disposed) return; disposed = true; abort.abort(); pause(); play.disabled = true; seek.disabled = true; volume.disabled = true; gain.disconnect(); void context.close().catch(() => { /* The detached player cannot report or restart playback. */ }); },
  };
}
