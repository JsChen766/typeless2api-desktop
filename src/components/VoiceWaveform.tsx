import { useMemo, useRef } from "react";
import { cn } from "@/lib/utils";

const BANDS = 11;
const IDLE_ENVELOPE = [0.12, 0.18, 0.28, 0.36, 0.42, 0.42, 0.36, 0.28, 0.18, 0.12, 0.08];
const SILENCE_BAR = 0.1;

function shapeLiveLevels(levels: number[]): number[] {
  const slice = levels.slice(0, BANDS);
  const avg = slice.reduce((sum, v) => sum + v, 0) / slice.length;
  if (avg < 0.07) {
    return slice.map(() => SILENCE_BAR);
  }
  const center = (BANDS - 1) / 2;
  return slice.map((v, i) => {
    if (v < 0.06) return SILENCE_BAR;
    const taper = 0.78 + 0.22 * (1 - Math.abs(i - center) / (center + 1));
    return SILENCE_BAR + Math.min(1, v * taper) * 0.9;
  });
}

type Props = {
  className?: string;
  barClassName?: string;
  maxHeightPx?: number;
  levels?: number[];
};

export function VoiceWaveform({
  className,
  barClassName = "bg-white",
  maxHeightPx = 22,
  levels,
}: Props) {
  const smoothRef = useRef<number[] | null>(null);
  const live = levels && levels.length >= BANDS;

  const heights = useMemo(() => {
    if (!live || !levels) return IDLE_ENVELOPE;
    const target = shapeLiveLevels(levels);
    const prev = smoothRef.current;
    if (!prev || prev.length !== BANDS) {
      smoothRef.current = target;
      return target;
    }
    const next = target.map((t, i) => {
      const alpha = t > prev[i] ? 0.55 : 0.72;
      return prev[i] * (1 - alpha) + t * alpha;
    });
    smoothRef.current = next;
    return next;
  }, [live, levels]);

  return (
    <div
      className={cn("flex h-7 items-center justify-center gap-[3px]", className)}
      aria-hidden
    >
      {heights.map((scale, i) => (
        <span
          key={i}
          className={cn(
            "w-[2.5px] shrink-0 rounded-full transition-[height] duration-75 ease-out",
            !live && "wave-pulse",
            barClassName
          )}
          style={{
            height: `${scale * maxHeightPx}px`,
            ...(!live
              ? {
                  animationDelay: `${i * 0.06}s`,
                  animationDuration: `${0.95 + (i % 3) * 0.12}s`,
                }
              : {}),
          }}
        />
      ))}
    </div>
  );
}
