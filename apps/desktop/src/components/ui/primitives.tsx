import { forwardRef, useId, useState, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from "react";
import { Switch as RSwitch, Tooltip as RTooltip } from "radix-ui";
import { Eye, EyeSlash, WarningCircle } from "@phosphor-icons/react";
import s from "./ui.module.css";

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

/** A quarter arc turning — the panel's own spinner. */
export function Spinner({ size = 14 }: { size?: number }) {
  return (
    <svg className={s.spinner} width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" strokeOpacity=".25" strokeWidth="2" />
      <path d="M8 1.8A6.2 6.2 0 0 1 14.2 8" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

type Variant = "primary" | "secondary" | "ghost" | "danger" | "dangerGhost";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "sm" | "md" | "lg";
  loading?: boolean;
  full?: boolean;
  icon?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", loading, full, icon, children, className, disabled, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type="button"
      className={cx(s.btn, s[variant], size !== "md" && s[size], full && s.full, className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? <Spinner /> : icon}
      {children}
    </button>
  );
});

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  shortcut?: string;
  active?: boolean;
  small?: boolean;
  tooltipSide?: "top" | "bottom" | "left" | "right";
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, shortcut, active, small, children, className, tooltipSide = "bottom", ...rest },
  ref,
) {
  return (
    <Tooltip content={label} shortcut={shortcut} side={tooltipSide}>
      <button
        ref={ref}
        type="button"
        aria-label={label}
        data-active={active || undefined}
        className={cx(s.iconBtn, small && s.iconBtnSm, className)}
        {...rest}
      >
        {children}
      </button>
    </Tooltip>
  );
});

export function Tooltip({
  content,
  shortcut,
  children,
  side = "bottom",
}: {
  content: ReactNode;
  shortcut?: string;
  children: ReactNode;
  side?: "top" | "bottom" | "left" | "right";
}) {
  return (
    <RTooltip.Root>
      <RTooltip.Trigger asChild>{children}</RTooltip.Trigger>
      <RTooltip.Portal>
        <RTooltip.Content className={s.tooltip} side={side} sideOffset={6} collisionPadding={8}>
          {content}
          {shortcut && <Kbd keys={shortcut} />}
        </RTooltip.Content>
      </RTooltip.Portal>
    </RTooltip.Root>
  );
}

export function Kbd({ keys }: { keys: string }) {
  return (
    <span style={{ display: "inline-flex", gap: 3 }}>
      {keys.split("+").map((k) => (
        <kbd key={k}>{k}</kbd>
      ))}
    </span>
  );
}

interface FieldShellProps {
  label?: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  id: string;
  children: ReactNode;
}

function FieldShell({ label, hint, error, id, children }: FieldShellProps) {
  return (
    <div className={s.field}>
      {label && (
        <label className={s.label} htmlFor={id}>
          {label}
        </label>
      )}
      {children}
      {error ? (
        <span className={s.error} id={`${id}-error`} role="alert">
          <WarningCircle size={14} weight="fill" style={{ flex: "none", marginTop: 1 }} />
          {error}
        </span>
      ) : (
        hint && (
          <span className={s.hint} id={`${id}-hint`}>
            {hint}
          </span>
        )
      )}
    </div>
  );
}

export interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  label?: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  mono?: boolean;
  large?: boolean;
  lead?: ReactNode;
  trailing?: ReactNode;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { label, hint, error, mono, large, lead, trailing, id, className, ...rest },
  ref,
) {
  const auto = useId();
  const fid = id ?? auto;
  return (
    <FieldShell label={label} hint={hint} error={error} id={fid}>
      <div className={cx(s.inputWrap, large && s.inputWrapLg)} data-invalid={!!error || undefined}>
        {lead && <span className={s.lead}>{lead}</span>}
        <input
          ref={ref}
          id={fid}
          className={cx(s.input, mono && s.inputMono, className)}
          aria-invalid={!!error || undefined}
          aria-describedby={error ? `${fid}-error` : hint ? `${fid}-hint` : undefined}
          spellCheck={false}
          autoComplete="off"
          {...rest}
        />
        {trailing && <span className={s.adornment}>{trailing}</span>}
      </div>
    </FieldShell>
  );
});

export interface PasswordFieldProps extends TextFieldProps {
  revealable?: boolean;
}

/** Password input. Uncontrolled-friendly: pass a ref and read `.value` on submit. */
export const PasswordField = forwardRef<HTMLInputElement, PasswordFieldProps>(function PasswordField(
  { revealable = true, trailing, ...rest },
  ref,
) {
  const [shown, setShown] = useState(false);
  return (
    <TextField
      ref={ref}
      type={shown ? "text" : "password"}
      mono={shown}
      trailing={
        <>
          {trailing}
          {revealable && (
            <IconButton
              small
              label={shown ? "Ocultar" : "Mostrar"}
              onClick={() => setShown((v) => !v)}
              tabIndex={-1}
              tooltipSide="top"
            >
              {shown ? <EyeSlash size={16} /> : <Eye size={16} />}
            </IconButton>
          )}
        </>
      }
      {...rest}
    />
  );
});

export interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  mono?: boolean;
}

export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(function TextArea(
  { label, hint, error, mono, id, className, ...rest },
  ref,
) {
  const auto = useId();
  const fid = id ?? auto;
  return (
    <FieldShell label={label} hint={hint} error={error} id={fid}>
      <div className={s.inputWrap} data-invalid={!!error || undefined}>
        <textarea
          ref={ref}
          id={fid}
          className={cx(s.input, s.textarea, mono && s.inputMono, className)}
          spellCheck={false}
          {...rest}
        />
      </div>
    </FieldShell>
  );
});

export function Switch({
  checked,
  onChange,
  disabled,
  id,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  id?: string;
  label?: string;
}) {
  return (
    <RSwitch.Root className={s.switch} checked={checked} onCheckedChange={onChange} disabled={disabled} id={id} aria-label={label}>
      <RSwitch.Thumb className={s.switchThumb} />
    </RSwitch.Root>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: ReactNode }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className={s.segmented} role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          data-active={value === o.value}
          className={s.segment}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export { cx };
