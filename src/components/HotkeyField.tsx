import { formatHotkeyLabel, keyboardEventToShortcut } from "@/lib/hotkey";
import { cn } from "@/lib/utils";
import { useEffect, useState } from "react";

const PRESETS = ["F9", "Ctrl+Alt+Z", "Ctrl+Shift+Space"] as const;
const COMMON_APP_SHORTCUTS = new Set([
  "Ctrl+A", "Ctrl+C", "Ctrl+F", "Ctrl+L", "Ctrl+N", "Ctrl+P",
  "Ctrl+Q", "Ctrl+R", "Ctrl+S", "Ctrl+T", "Ctrl+V", "Ctrl+W", "Ctrl+X", "Ctrl+Z",
]);

type Props = {
  value: string;
  onChange: (shortcut: string) => void;
  onCapturingChange?: (capturing: boolean) => void;
  className?: string;
};

export function HotkeyField({ value, onChange, onCapturingChange, className }: Props) {
  const [listening, setListening] = useState(false);
  const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);

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
    <div className="space-y-1.5">
      <button
        type="button"
        onClick={() => setListening(true)}
        aria-label={listening ? "按下新快捷键，Esc 取消" : `当前快捷键 ${formatHotkeyLabel(value)}，点击修改`}
        className={cn(
          "flex h-9 w-full items-center justify-between rounded-[8px] border border-[#34414a] bg-[#10161b] px-2.5 text-left text-[13px] text-[#f1f5f4] outline-none",
          "hover:border-[#657c80] focus-visible:border-[#7bd5be] focus-visible:ring-2 focus-visible:ring-[#7bd5be]/20",
          listening && "border-[#7bd5be] ring-2 ring-[#7bd5be]/20",
          className
        )}
      >
        {listening ? (
          <span className="text-[12px] text-[#b7c8c7]">按下新快捷键，Esc 取消</span>
        ) : (
          <>
            <kbd className="font-sans font-medium not-italic">{formatHotkeyLabel(value)}</kbd>
            <span className="text-[11px] text-[#8fa3a7]">修改</span>
          </>
        )}
      </button>
      <div className="flex flex-wrap gap-1.5" aria-label="快捷键预设">
        {PRESETS.map((preset) => (
          <button
            key={preset}
            type="button"
            onClick={() => {
              setListening(false);
              onChange(preset);
            }}
            aria-pressed={value === preset}
            className={cn(
              "rounded-[6px] border px-2 py-1 text-[10px] leading-[16px] outline-none focus-visible:ring-2 focus-visible:ring-[#7bd5be]",
              value === preset
                ? "border-[#4a8b7d] bg-[#203b38] text-[#a6e9d4]"
                : "border-[#334047] text-[#aab9bd] hover:border-[#657c80] hover:text-[#e5ecea]"
            )}
          >
            {formatHotkeyLabel(preset)}
          </button>
        ))}
      </div>
      {isMac && value === "F9" ? (
        <p className="text-[10px] text-[#90a2a7]">部分 Mac 键盘需按 Fn + F9</p>
      ) : null}
      {COMMON_APP_SHORTCUTS.has(value) ? (
        <p className="text-[10px] leading-[1.4] text-[#e2a190]">
          {formatHotkeyLabel(value)} 常被其他应用使用，也会触发听写；建议改用 F9。
        </p>
      ) : null}
    </div>
  );
}
