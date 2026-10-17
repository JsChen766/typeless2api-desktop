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
        "relative h-6 w-10 shrink-0 rounded-full border border-foreground/15 transition-colors",
        checked ? "bg-foreground/90" : "bg-foreground/10"
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-background shadow transition-transform",
          checked && "translate-x-4"
        )}
      />
    </button>
  );
}
