import { Check, Loader2, X } from "lucide-react";
import { VoiceWaveform } from "@/components/VoiceWaveform";
import { cn } from "@/lib/utils";

type Props = {
  visible: boolean;
  processing?: boolean;
  onCancel: () => void;
  onComplete: () => void;
  className?: string;
};

export function DictationPill({
  visible,
  processing,
  onCancel,
  onComplete,
  className,
}: Props) {
  if (!visible) return null;
  return (
    <div
      className={cn(
        "fixed bottom-8 left-1/2 z-50 flex -translate-x-1/2 items-center gap-1 rounded-full border border-zinc-600/90 bg-black px-1.5 py-1.5 shadow-[0_16px_48px_rgba(0,0,0,0.35)]",
        className
      )}
      role="status"
      aria-label="听写进行中"
    >
      <button
        type="button"
        onClick={onCancel}
        className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-zinc-700 text-white transition-colors hover:bg-zinc-600 disabled:opacity-50"
        aria-label="取消"
      >
        <X className="h-4 w-4" strokeWidth={2.25} />
      </button>
      <div className="flex w-[132px] items-center justify-center px-1">
        {processing ? (
          <Loader2 className="h-5 w-5 animate-spin text-zinc-400" />
        ) : (
          <VoiceWaveform />
        )}
      </div>
      <button
        type="button"
        disabled={processing}
        onClick={onComplete}
        className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-white text-black transition-colors hover:bg-zinc-100 disabled:opacity-50"
        aria-label="完成"
      >
        <Check className="h-4 w-4" strokeWidth={2.5} />
      </button>
    </div>
  );
}
