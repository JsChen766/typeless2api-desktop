import { cn } from "@/lib/utils";

/** 对称波包：中间高、两侧低，绕水平中轴伸缩（参考 Typeless 状态条） */
const ENVELOPE = [0.22, 0.38, 0.58, 0.78, 0.92, 1, 0.92, 0.78, 0.58, 0.38, 0.22];

type Props = {
  className?: string;
  barClassName?: string;
  maxHeightPx?: number;
};

export function VoiceWaveform({
  className,
  barClassName = "bg-white",
  maxHeightPx = 26,
}: Props) {
  return (
    <div
      className={cn("flex h-9 items-center justify-center gap-[5px]", className)}
      aria-hidden
    >
      {ENVELOPE.map((scale, i) => (
        <span
          key={i}
          className={cn("wave-pulse w-[2.5px] shrink-0 rounded-full", barClassName)}
          style={{
            height: `${scale * maxHeightPx}px`,
            animationDelay: `${i * 0.06}s`,
            animationDuration: `${0.95 + (i % 3) * 0.12}s`,
          }}
        />
      ))}
    </div>
  );
}
