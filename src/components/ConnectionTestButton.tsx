import { Check, Loader2, RefreshCw } from "lucide-react";

type Status = "idle" | "loading" | "ok" | "fail";

type Props = {
  status: Status;
  onClick: () => void;
};

export function ConnectionTestButton({ status, onClick }: Props) {
  const loading = status === "loading";

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={loading}
      className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-[8px] border border-[#3a474e] bg-[#222c32] px-2.5 text-[11px] font-medium text-[#e1ebe8] outline-none hover:border-[#77988f] hover:bg-[#2b373c] focus-visible:ring-2 focus-visible:ring-[#7bd5be] disabled:cursor-wait disabled:opacity-70"
    >
      {loading ? (
        <Loader2 size={13} className="animate-spin motion-reduce:animate-none" aria-hidden="true" />
      ) : status === "ok" ? (
        <Check size={13} aria-hidden="true" />
      ) : (
        <RefreshCw size={13} aria-hidden="true" />
      )}
      {loading ? "检测中…" : "测试连接"}
    </button>
  );
}
