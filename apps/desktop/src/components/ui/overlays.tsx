import type { ReactNode } from "react";
import { ContextMenu as RContext, Dialog as RDialog, DropdownMenu as RMenu } from "radix-ui";
import s from "./ui.module.css";
import { Button } from "./primitives";

// ---------- Dialog ----------

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  wide,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}) {
  return (
    <RDialog.Root open={open} onOpenChange={onOpenChange}>
      <RDialog.Portal>
        <RDialog.Overlay className={s.overlay} />
        <RDialog.Content className={`${s.dialog} ${wide ? s.dialogWide : ""}`} aria-describedby={description ? undefined : undefined}>
          <div className={s.dialogBody}>
            <RDialog.Title className={s.dialogTitle}>{title}</RDialog.Title>
            {description ? (
              <RDialog.Description className={s.dialogDesc}>{description}</RDialog.Description>
            ) : (
              <RDialog.Description className="sr-only">{typeof title === "string" ? title : ""}</RDialog.Description>
            )}
            {children}
          </div>
          {footer && <div className={s.dialogFooter}>{footer}</div>}
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}

/** A confirmation that states the consequence in plain words. */
export function Confirm({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  danger,
  onConfirm,
  busy,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: ReactNode;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
  busy?: boolean;
  children?: ReactNode;
}) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      description={description}
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button variant={danger ? "danger" : "primary"} onClick={onConfirm} loading={busy} autoFocus>
            {confirmLabel}
          </Button>
        </>
      }
    >
      {children}
    </Dialog>
  );
}

// ---------- Menus ----------

export interface MenuEntry {
  label?: string;
  icon?: ReactNode;
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  onSelect?: () => void;
  separator?: boolean;
  heading?: string;
}

function entries(Item: typeof RMenu.Item, Sep: typeof RMenu.Separator, Label: typeof RMenu.Label, list: MenuEntry[]) {
  return list.map((e, i) => {
    if (e.separator) return <Sep key={i} className={s.menuSep} />;
    if (e.heading) return <Label key={i} className={s.menuLabel}>{e.heading}</Label>;
    return (
      <Item key={i} className={`${s.menuItem} ${e.danger ? s.menuItemDanger : ""}`} disabled={e.disabled} onSelect={e.onSelect}>
        {e.icon}
        {e.label}
        {e.shortcut && <span className={s.menuShortcut}>{e.shortcut}</span>}
      </Item>
    );
  });
}

export function Menu({
  trigger,
  items,
  align = "end",
}: {
  trigger: ReactNode;
  items: MenuEntry[];
  align?: "start" | "center" | "end";
}) {
  return (
    <RMenu.Root>
      <RMenu.Trigger asChild>{trigger}</RMenu.Trigger>
      <RMenu.Portal>
        <RMenu.Content className={s.menu} align={align} sideOffset={6} collisionPadding={8}>
          {entries(RMenu.Item, RMenu.Separator, RMenu.Label, items)}
        </RMenu.Content>
      </RMenu.Portal>
    </RMenu.Root>
  );
}

export function ContextMenu({ children, items }: { children: ReactNode; items: MenuEntry[] }) {
  return (
    <RContext.Root>
      <RContext.Trigger asChild>{children}</RContext.Trigger>
      <RContext.Portal>
        <RContext.Content className={s.menu} collisionPadding={8}>
          {entries(RContext.Item as unknown as typeof RMenu.Item, RContext.Separator as unknown as typeof RMenu.Separator, RContext.Label as unknown as typeof RMenu.Label, items)}
        </RContext.Content>
      </RContext.Portal>
    </RContext.Root>
  );
}
