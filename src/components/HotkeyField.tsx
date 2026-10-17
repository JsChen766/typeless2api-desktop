import { formatHotkeyLabel, keyboardEventToShortcut } from "@/lib/hotkey";
import { cn } from "@/lib/utils";
import { useEffect, useState } from "react";

const PRESETS = ["F9", "Ctrl+Alt+Z", "Ctrl+Shift+Space"] as const;

type Props = {
  value: string;
  onChange: (shortcut: string) => void;
  onCapturingChange?: (capturing: boolean) => void;
  className?: string;
};

export function HotkeyField({ value, onChange, onCapturingChange, className }: Props) {
  const [listening, setListening] = useState(false);

  useEffect(() => {
    onCapturingChange?.(listening);
  }, [listening, onCapturingChange]);

  useEffect(() => {
    if (!listening) return;
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setListening(false);
        return;
      }
      const shortcut = keyboardEventToShortcut(e);
      if (!shortcut) return;
      onChange(shortcut);
      setListening(false);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [listening, onChange]);

  return (
    <div className="space-y-2">
    <button
      type="button"
      onClick={() => setListening(true)}
      className={cn(
        "w-full rounded-lg border border-foreground/12 bg-transparent px-3 py-2.5 text-left text-sm text-foreground outline-none transition-colors",
        "hover:border-foreground/25 focus:border-foreground/35 focus:ring-2 focus:ring-foreground/10",
        listening && "border-foreground/40 ring-2 ring-foreground/10",
        className
      )}
    >
      {listening ? (
        <span className="text-muted-foreground">按下新快捷键（Esc 取消）…</span>
      ) : (
        <kbd className="font-sans not-italic">{formatHotkeyLabel(value)}</kbd>
      )}
    </button>
    <div className="flex flex-wrap gap-2">
      {PRESETS.map((p) => (
        <button
          key={p}
          type="button"
          onClick={() => onChange(p)}
          className="rounded-md border border-foreground/10 px-2 py-1 text-[11px] text-muted-foreground hover:border-foreground/25 hover:text-foreground"
        >
          {p.replace(/CommandOrControl/g, "Ctrl")}
        </button>
      ))}
    </div>
    </div>
  );
}
