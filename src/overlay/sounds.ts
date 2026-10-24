// Short synthesized cues, so no audio files ship with the app.
let context: AudioContext | null = null;

function chime(frequencies: number[], volume = 0.07) {
  try {
    context ??= new AudioContext();
    void context.resume();
    const ctx = context;
    const start = ctx.currentTime + 0.01;
    frequencies.forEach((frequency, i) => {
      const at = start + i * 0.075;
      const oscillator = ctx.createOscillator();
      const gain = ctx.createGain();
      oscillator.type = "sine";
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0, at);
      gain.gain.linearRampToValueAtTime(volume, at + 0.012);
      gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.18);
      oscillator.connect(gain).connect(ctx.destination);
      oscillator.start(at);
      oscillator.stop(at + 0.2);
    });
  } catch {
    // Sound is a nicety; a webview without audio just stays quiet.
  }
}

export const sounds = {
  start: () => chime([740, 988]),
  stop: () => chime([988, 740]),
  error: () => chime([330, 247], 0.06),
  hint: () => chime([660], 0.045),
};
