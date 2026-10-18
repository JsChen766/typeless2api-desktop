import { cn } from "@/lib/utils";

type Props = {
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
  "aria-label": string;
};

export function Toggle({ checked, onCheckedChange, "aria-label": ariaLabel }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "relative h-6 w-10 shrink-0 rounded-full border outline-none transition-colors focus-visible:ring-2 focus-visible:ring-[#7bd5be] focus-visible:ring-offset-2 focus-visible:ring-offset-[#171d23]",
        checked ? "border-[#7bd5be] bg-[#7bd5be]" : "border-[#46535a] bg-[#333d43]"
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 left-0.5 h-[18px] w-[18px] rounded-full bg-[#eef5f1] shadow-sm transition-transform",
          checked && "translate-x-[18px]"
        )}
      />
    </button>
  );
}
