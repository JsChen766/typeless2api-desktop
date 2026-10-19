import { Check, Loader2, X } from "lucide-react";
import { VoiceWaveform } from "@/components/VoiceWaveform";
import { cn } from "@/lib/utils";

type Props = {
  visible: boolean;
  processing?: boolean;
  error?: string | null;
  micLevels?: number[];
  onCancel: () => void;
  onComplete: () => void;
  className?: string;
};

export function DictationPill({
  visible,
  processing,
  error,
  micLevels,
  onCancel,
  onComplete,
  className,
}: Props) {
  if (!visible) return null;
  return (
    <div
      className={cn(
        "fixed bottom-8 left-1/2 z-50 flex -translate-x-1/2 items-center gap-0.5 rounded-full border border-zinc-600/90 bg-black px-1 py-1 shadow-[0_12px_36px_rgba(0,0,0,0.35)]",
        className
      )}
      role="status"
      aria-label={error ? `听写失败：${error}` : "听写进行中"}
    >
      <button
        type="button"
        onClick={onCancel}
        className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-zinc-700 text-white transition-colors hover:bg-zinc-600 disabled:opacity-50"
        aria-label="取消"
      >
        <X className="h-3.5 w-3.5" strokeWidth={2.25} />
      </button>
      <div
        className={cn(
          "flex items-center justify-center px-0.5",
          error ? "w-[156px]" : "w-[96px]"
        )}
      >
        {error ? (
          <span
            className="line-clamp-2 text-center text-[10px] leading-3.5 text-rose-200"
            title={error}
          >
            {error}
          </span>
        ) : processing ? (
          <Loader2 className="h-4 w-4 animate-spin text-zinc-400" />
        ) : (
          <VoiceWaveform levels={micLevels} />
        )}
      </div>
      {!error ? (
        <button
          type="button"
          disabled={processing}
          onClick={onComplete}
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-white text-black transition-colors hover:bg-zinc-100 disabled:opacity-50"
          aria-label="完成"
        >
          <Check className="h-3.5 w-3.5" strokeWidth={2.5} />
        </button>
      ) : null}
    </div>
  );
}
