// Typed bridge to the Rust side. There is no JavaScript fallback: every call reaches the
// real core, including in development (demo data is seeded through the real API by a
// debug-only command that release builds do not contain).

import type {
  AppInfo,
  CloudMe,
  CloudStatus,
  SyncOutcome,
  BreachReport,
  ExportResult,
  ImportPreview,
  ImportSource,
  HealthReport,
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
  UpdateInfo,
  Uuid,
  VaultAttrs,
  VaultInfo,
  VersionView,
} from "./types";

export type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
export type Listen = (event: string, handler: (payload: unknown) => void) => Promise<() => void>;

let invokeImpl: Invoke;
let listenImpl: Listen;
let markReady: () => void;
const ready = new Promise<void>((r) => (markReady = r));

export async function initBridge(): Promise<void> {
  const core = await import("@tauri-apps/api/core");
  const event = await import("@tauri-apps/api/event");
  invokeImpl = core.invoke as Invoke;
  listenImpl = async (name, handler) => event.listen(name, (e) => handler(e.payload));
  markReady();
}

// Calls made before the bridge is up (module-level code) wait for it instead of failing.
const call = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  if (!invokeImpl) await ready;
  return invokeImpl<T>(cmd, args);
};

export const on = async (event: string, handler: (payload: unknown) => void) => {
  if (!listenImpl) await ready;
  return listenImpl(event, handler);
};

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  createAccount: (password: string, vaultName?: string) =>
    call<CreatedAccount>("account_create", { args: { password, vaultName } }),
  unlock: (password: string, secretKey?: string) => call<void>("account_unlock", { args: { password, secretKey } }),
  lock: () => call<void>("account_lock"),
  unlockHello: () => call<void>("account_unlock_hello"),
  helloEnable: (password: string) => call<void>("hello_enable", { args: { password } }),
  helloDisable: () => call<void>("hello_disable"),
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

  health: () => call<HealthReport>("health_report"),
  breachCheck: () => call<BreachReport>("breach_check"),

  importPick: (source: ImportSource) => call<ImportPreview | null>("import_pick", { source }),
  importBackupPick: (password: string) => call<ImportPreview | null>("import_backup_pick", { args: { password } }),
  importCommit: (args: { vaultId: Uuid; skipDuplicates: boolean; foldersAsTags: boolean; tag: string | null; exclude: number[] }) =>
    call<{ imported: number; skipped: number }>("import_commit", { args }),
  importCancel: () => call<void>("import_cancel"),
  exportBackup: (masterPassword: string, exportPassword: string) =>
    call<ExportResult | null>("export_backup", { args: { masterPassword, exportPassword } }),
  exportCsv: (password: string) => call<ExportResult | null>("export_csv", { args: { password } }),

  quickHide: () => call<void>("quick_hide"),
  quickOpenInMain: (itemId: Uuid) => call<void>("quick_open_in_main", { itemId }),

  updateCheck: () => call<UpdateInfo | null>("update_check"),
  updateInstall: () => call<void>("update_install"),

  attachmentAdd: (itemId: Uuid) => call<ItemView | null>("attachment_add", { itemId }),
  attachmentAddPaths: (itemId: Uuid, paths: string[]) => call<ItemView>("attachment_add_paths", { itemId, paths }),
  attachmentSave: (itemId: Uuid, attachmentId: Uuid) => call<string | null>("attachment_save", { itemId, attachmentId }),
  attachmentPreview: (itemId: Uuid, attachmentId: Uuid) => call<string>("attachment_preview", { itemId, attachmentId }),
  attachmentRemove: (itemId: Uuid, attachmentId: Uuid) => call<ItemView>("attachment_remove", { itemId, attachmentId }),
  wifiQr: (itemId: Uuid) => call<string>("wifi_qr", { itemId }),

  cloudStatus: () => call<CloudStatus>("cloud_status"),
  cloudSignup: (email: string, password: string) => call<void>("cloud_signup", { args: { email, password } }),
  cloudSignin: (email: string, password: string, secretKey: string, totp?: string) =>
    call<void>("cloud_signin", { args: { email, password, secretKey, totp: totp || null } }),
  cloudSyncNow: () => call<SyncOutcome>("cloud_sync_now"),
  cloudSignout: () => call<void>("cloud_signout"),
  cloudMe: () => call<CloudMe>("cloud_me"),
  cloudRevokeDevice: (id: Uuid) => call<void>("cloud_revoke_device", { id }),
  cloudTotpSetup: () => call<{ secret: string; uri: string }>("cloud_totp_setup"),
  cloudTotpEnable: (secret: string, code: string) => call<{ recoveryCodes: string[] }>("cloud_totp_enable", { secret, code }),
  cloudTotpDisable: (code: string) => call<void>("cloud_totp_disable", { code }),
  cloudDeleteAccount: (password: string) => call<void>("cloud_delete_account", { password }),

  settings: () => call<Settings>("settings_get"),
  updateSettings: (settings: Settings) => call<Settings>("settings_update", { settings }),
  openUrl: (url: string) => call<void>("open_url", { url }),
  appVisibility: (visible: boolean) => call<void>("app_visibility", { visible }),
  appBackground: () => call<void>("app_background"),
  appInsets: () => call<{ top: number; bottom: number; left: number; right: number; keyboard: number } | null>("app_insets"),
  appBarStyle: (dark: boolean) => call<void>("app_bar_style", { dark }),
  appPrint: (title: string) => call<void>("app_print", { title }),
};

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return typeof e === "string" ? e : "Algo deu errado.";
}

export function errorCode(e: unknown): string | null {
  if (e && typeof e === "object" && "code" in e) return String((e as { code: unknown }).code);
  return null;
}
