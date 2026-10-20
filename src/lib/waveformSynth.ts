const BANDS = 11;
const SILENCE_BAR = 0.1;

function bandProfile(i: number): { heightMul: number } {
  const center = (BANDS - 1) / 2;
  const t = Math.abs(i - center) / center;
  const falloff = Math.pow(1 - t, 2.1);
  return { heightMul: 0.14 + 0.86 * falloff };
}

/**
 * 把 Rust 推来的 0..1 电平，映射成「低段敏感、高段压缩」的视觉强度。
 * 对应你说的约 0–500 起伏大、500–700 再往上振幅变小（按 700 封顶示意）。
 */
export function driveFromLevel(master: number): number {
  const clamped = Math.min(1, Math.max(0, master));
  const pseudoPeak = clamped * 700;
  if (pseudoPeak <= 500) {
    return (pseudoPeak / 500) * 0.82;
  }
  return 0.82 + ((pseudoPeak - 500) / 200) * 0.18;
}

export function macSyntheticHeights(drive: number, t: number): number[] {
  if (drive < 0.03) {
    return Array.from({ length: BANDS }, () => SILENCE_BAR);
  }
  const wobbleScale = 0.42 * (1 - drive * 0.5);
  return Array.from({ length: BANDS }, (_, i) => {
    const { heightMul } = bandProfile(i);
    const wobble =
      wobbleScale *
      (Math.sin(t * 2.6 + i * 0.62) * 0.5 +
        Math.sin(t * 5.8 + i * 1.35) * 0.32 +
        Math.sin(t * 10.5 + i * 2.05) * 0.18);
    const body = drive * heightMul + wobble * heightMul;
    return SILENCE_BAR + Math.min(0.92, Math.max(0, body));
  });
}

export function masterFromLevels(levels: number[]): number {
  const slice = levels.slice(0, BANDS);
  if (slice.length === 0) return 0;
  const peak = Math.max(...slice, 0);
  const avg = slice.reduce((s, v) => s + v, 0) / slice.length;
  return Math.max(peak, avg * 1.15);
}
