// Typed bridge to the Rust side. There is no JavaScript fallback: every call reaches the
// real core, including in development (demo data is seeded through the real API by a
// debug-only command that release builds do not contain).

import type {
  AppInfo,
  CopyResult,
  CreatedAccount,
  Generated,
  ItemFull,
  ItemInput,
  ItemsPayload,
  ItemView,
  Recipe,
  SecurityStatus,
  Settings,
  Strength,
  TotpCode,
  Uuid,
  VaultAttrs,
  VaultInfo,
  VersionView,
} from "./types";

export type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
export type Listen = (event: string, handler: (payload: unknown) => void) => Promise<() => void>;

let invokeImpl: Invoke;
let listenImpl: Listen;

export async function initBridge(): Promise<void> {
  const core = await import("@tauri-apps/api/core");
  const event = await import("@tauri-apps/api/event");
  invokeImpl = core.invoke as Invoke;
  listenImpl = async (name, handler) => event.listen(name, (e) => handler(e.payload));
}

const call = <T>(cmd: string, args?: Record<string, unknown>) => invokeImpl<T>(cmd, args);

export const on = (event: string, handler: (payload: unknown) => void) => listenImpl(event, handler);

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  createAccount: (password: string, vaultName?: string) =>
    call<CreatedAccount>("account_create", { args: { password, vaultName } }),
  unlock: (password: string, secretKey?: string) => call<void>("account_unlock", { args: { password, secretKey } }),
  lock: () => call<void>("account_lock"),
  recover: (recoveryCode: string, newPassword: string, secretKey?: string) =>
    call<string>("account_recover", { args: { recoveryCode, newPassword, secretKey } }),
  security: () => call<SecurityStatus>("account_security"),
  revealSecretKey: (password: string) => call<string>("secret_key_reveal", { args: { password } }),
  changePassword: (current: string, next: string) => call<void>("password_change", { args: { current, next } }),
  rotateRecovery: (password: string) => call<string>("recovery_rotate", { args: { password } }),
  disableRecovery: () => call<void>("recovery_disable"),

  vaults: () => call<VaultInfo[]>("vaults_list"),
  createVault: (attrs: VaultAttrs) => call<VaultInfo>("vault_create", { attrs }),
  updateVault: (id: Uuid, attrs: VaultAttrs) => call<VaultInfo>("vault_update", { id, attrs }),
  deleteVault: (id: Uuid, moveTo: Uuid | null) => call<void>("vault_delete", { id, moveTo }),

  items: () => call<ItemsPayload>("items_list"),
  item: (id: Uuid) => call<ItemView>("item_get", { id }),
  itemForEdit: (id: Uuid) => call<ItemFull>("item_get_for_edit", { id }),
  createItem: (vaultId: Uuid, input: ItemInput) => call<ItemView>("item_create", { vaultId, input }),
  updateItem: (id: Uuid, input: ItemInput) => call<ItemView>("item_update", { id, input }),
  setFavorite: (id: Uuid, favorite: boolean) => call<void>("item_set_favorite", { id, favorite }),
  setArchived: (id: Uuid, archived: boolean) => call<void>("item_set_archived", { id, archived }),
  trash: (id: Uuid) => call<void>("item_trash", { id }),
  restore: (id: Uuid) => call<void>("item_restore", { id }),
  purge: (id: Uuid) => call<void>("item_purge", { id }),
  emptyTrash: () => call<number>("trash_empty"),
  move: (id: Uuid, vaultId: Uuid) => call<void>("item_move", { id, vaultId }),
  duplicate: (id: Uuid) => call<ItemView>("item_duplicate", { id }),
  history: (id: Uuid) => call<VersionView[]>("item_history", { id }),
  restoreVersion: (id: Uuid, revision: number) => call<ItemView>("item_restore_version", { id, revision }),
  touch: (id: Uuid) => call<void>("item_touch", { id }),

  reveal: (itemId: Uuid, field: string) => call<string>("field_reveal", { itemId, field }),
  copyField: (itemId: Uuid, field: string) => call<CopyResult>("field_copy", { itemId, field }),
  copyText: (text: string, sensitive: boolean) => call<CopyResult>("copy_text", { args: { text, sensitive } }),
  totp: (itemId: Uuid, field: string) => call<TotpCode>("totp_now", { itemId, field }),
  totpPreview: (secret: string) => call<TotpCode>("totp_preview", { args: { secret } }),

  generate: (recipe: Recipe) => call<Generated>("generator_generate", { recipe }),
  strength: (password: string, context: string[] = []) =>
    call<Strength>("strength_estimate", { args: { password, context } }),

  settings: () => call<Settings>("settings_get"),
  updateSettings: (settings: Settings) => call<Settings>("settings_update", { settings }),
  openUrl: (url: string) => call<void>("open_url", { url }),
};

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return typeof e === "string" ? e : "Algo deu errado.";
}

export function errorCode(e: unknown): string | null {
  if (e && typeof e === "object" && "code" in e) return String((e as { code: unknown }).code);
  return null;
}
