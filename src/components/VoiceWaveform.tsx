import { useMemo, useRef } from "react";
import { cn } from "@/lib/utils";

const BANDS = 11;
const IDLE_ENVELOPE = [0.12, 0.18, 0.28, 0.36, 0.42, 0.42, 0.36, 0.28, 0.18, 0.12, 0.08];
const SILENCE_BAR = 0.1;

/** 中间高、两侧低（宽度 + 高度上限），不做对称“阶梯” */
function bandProfile(i: number): { heightMul: number; widthPx: number } {
  const center = (BANDS - 1) / 2;
  const t = Math.abs(i - center) / center;
  const falloff = Math.pow(1 - t, 2.1);
  return {
    heightMul: 0.14 + 0.86 * falloff,
    widthPx: 1.35 + 1.45 * falloff,
  };
}

function compressBar(v: number): number {
  if (v < 0.045) return 0;
  const x = Math.min(1, (v - 0.045) * 2.2);
  return Math.pow(x, 0.68) * 0.9;
}

/**
 * 11 路电平是麦克风线程写入的时间环，每根应跟自己的采样动，
 * 再用包络压成中间高、两边低，而不是一根总 drive 画对称三角。
 */
function shapeLiveLevels(levels: number[]): number[] {
  const slice = levels.slice(0, BANDS);
  const peak = Math.max(...slice, 0);
  const avg = slice.reduce((sum, v) => sum + v, 0) / slice.length;
  if (peak < 0.05 && avg < 0.04) {
    return slice.map(() => SILENCE_BAR);
  }

  return slice.map((v, i) => {
    const { heightMul } = bandProfile(i);
    const amp = compressBar(v);
    // 相对整窗平均的起伏：说话时各时刻电平不同，条高会错落
    const ripple = (v - avg) * 1.55 * heightMul;
    const neighbor = slice[Math.max(0, i - 1)];
    const edge = Math.abs(v - neighbor) * 0.85 * heightMul;
    const body = amp * heightMul + ripple + edge;
    return SILENCE_BAR + Math.min(0.94, Math.max(0, body));
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
      const rising = t > prev[i];
      const alpha = rising ? 0.78 : 0.42;
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
      {heights.map((scale, i) => {
        const { widthPx } = bandProfile(i);
        return (
          <span
            key={i}
            className={cn(
              "shrink-0 rounded-full transition-[height,width] duration-50 ease-out",
              !live && "w-[2.5px] wave-pulse",
              barClassName
            )}
            style={{
              width: live ? `${widthPx}px` : undefined,
              height: `${scale * maxHeightPx}px`,
              ...(!live
                ? {
                    animationDelay: `${i * 0.06}s`,
                    animationDuration: `${0.95 + (i % 3) * 0.12}s`,
                  }
                : {}),
            }}
          />
        );
      })}
    </div>
  );
}
