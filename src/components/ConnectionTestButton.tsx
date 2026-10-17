import { cn } from "@/lib/utils";
import { Check, KeyRound, Loader2 } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

type Status = "idle" | "loading" | "ok" | "fail";

type Props = {
  status: Status;
  onClick: () => void;
};

export function ConnectionTestButton({ status, onClick }: Props) {
  const ok = status === "ok";
  const loading = status === "loading";
  const fail = status === "fail";

  return (
    <motion.button
      type="button"
      layout
      onClick={onClick}
      disabled={loading}
      className={cn(
        "inline-flex h-9 items-center overflow-hidden rounded-lg border px-3 text-sm font-medium",
        "disabled:cursor-wait disabled:opacity-80"
      )}
      animate={{
        borderColor: ok
          ? "oklch(0.95 0 0 / 0.45)"
          : fail
            ? "oklch(0.55 0 0 / 0.5)"
            : "oklch(0.95 0 0 / 0.14)",
        backgroundColor: ok
          ? "oklch(0.95 0 0 / 0.08)"
          : fail
            ? "oklch(0.35 0 0 / 0.35)"
            : "transparent",
      }}
      transition={{ type: "spring", stiffness: 420, damping: 32 }}
      whileTap={{ scale: 0.97 }}
    >
      <motion.span layout className="flex items-center gap-2">
        <AnimatePresence mode="popLayout" initial={false}>
          {loading ? (
            <motion.span
              key="load"
              initial={{ opacity: 0, scale: 0.6 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.6 }}
            >
              <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
            </motion.span>
          ) : ok ? (
            <motion.span
              key="check"
              initial={{ opacity: 0, scale: 0.5, rotate: -40 }}
              animate={{ opacity: 1, scale: 1, rotate: 0 }}
              exit={{ opacity: 0, scale: 0.5 }}
              transition={{ type: "spring", stiffness: 500, damping: 28 }}
            >
              <Check className="h-3.5 w-3.5 text-foreground" strokeWidth={2.5} />
            </motion.span>
          ) : (
            <motion.span
              key="key"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
            >
              <KeyRound className="h-3.5 w-3.5 text-muted-foreground" />
            </motion.span>
          )}
        </AnimatePresence>

        <span className="relative block min-w-[4.5rem] text-left">
          <AnimatePresence mode="wait" initial={false}>
            <motion.span
              key={status}
              className="block"
              initial={{ opacity: 0, y: 8, filter: "blur(6px)" }}
              animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
              exit={{ opacity: 0, y: -8, filter: "blur(6px)" }}
              transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            >
              {loading ? "连接中…" : ok ? "已连接" : fail ? "连接失败" : "测试连接"}
            </motion.span>
          </AnimatePresence>
        </span>
      </motion.span>
    </motion.button>
  );
}
