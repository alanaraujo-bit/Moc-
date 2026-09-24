// Mirrors of the Rust DTOs (apps/desktop/src-tauri/src/dto.rs, crates/moco-core/src/model.rs).

export type Uuid = string;
export type Timestamp = number; // unix ms

export type ItemKind =
  | "login"
  | "password"
  | "card"
  | "identity"
  | "document"
  | "note"
  | "wifi"
  | "license"
  | "bankAccount"
  | "server"
  | "database"
  | "apiCredential"
  | "sshKey"
  | "cryptoWallet"
  | "healthPlan"
  | "vehicle"
  | "custom";

export type FieldType =
  | "text"
  | "concealed"
  | "email"
  | "url"
  | "phone"
  | "date"
  | "monthYear"
  | "number"
  | "pin"
  | "totp"
  | "multiline"
  | "select"
  | "cardNumber"
  | "cpf"
  | "cnpj"
  | "cep"
  | "sshPrivateKey"
  | "seedPhrase";

export const CONCEALED_TYPES: ReadonlySet<FieldType> = new Set(["concealed", "pin", "totp", "sshPrivateKey", "seedPhrase"]);

export interface Settings {
  theme: "system" | "light" | "dark";
  autoLockMinutes: number;
  lockOnSleep: boolean;
  lockOnSessionLock: boolean;
  lockOnMinimize: boolean;
  clipboardClearSeconds: number;
  closeToTray: boolean;
  launchAtStartup: boolean;
  quickAccessShortcut: string;
  screenCaptureProtection: boolean;
  breachCheck: boolean;
  siteIcons: boolean;
  requirePasswordDays: number;
  updateChannel: "stable" | "beta";
  autoUpdate: boolean;
  density: "comfortable" | "compact";
  dismissedTips: string[];
  lastSeenVersion: string;
}

export interface AppError {
  code: string;
  message: string;
}

export interface AppInfo {
  version: string;
  initialized: boolean;
  unlocked: boolean;
  secretKeyOnDevice: boolean;
  storageError: AppError | null;
  settings: Settings;
  debug: boolean;
  helloAvailable: boolean;
  helloEnrolled: boolean;
  passwordDue: boolean;
}

export interface CreatedAccount {
  secretKey: string;
  recoveryCode: string;
}

export interface VaultAttrs {
  name: string;
  description: string;
  icon: string;
  color: string;
}

export interface VaultInfo extends VaultAttrs {
  id: Uuid;
  createdAt: Timestamp;
  updatedAt: Timestamp;
  itemCount: number;
}

export interface ItemSummary {
  id: Uuid;
  vaultId: Uuid;
  kind: ItemKind;
  title: string;
  subtitle: string;
  urls: string[];
  tags: string[];
  favorite: boolean;
  archived: boolean;
  trashedAt: Timestamp | null;
  icon: string | null;
  keywords: string[];
  hasTotp: boolean;
  hasPasskey: boolean;
  attachmentCount: number;
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

export interface Usage {
  lastUsedAt: Timestamp;
  count: number;
}

export interface ItemsPayload {
  items: ItemSummary[];
  usage: Record<Uuid, Usage>;
}

export interface FieldView {
  id: string;
  label: string;
  type: FieldType;
  value: string | null;
  hasValue: boolean;
  section: string | null;
  strength: number | null;
}

export interface Section {
  id: string;
  label: string;
}

export interface ItemView {
  id: Uuid;
  vaultId: Uuid;
  kind: ItemKind;
  title: string;
  subtitle: string;
  urls: string[];
  tags: string[];
  favorite: boolean;
  archived: boolean;
  trashedAt: Timestamp | null;
  icon: string | null;
  fields: FieldView[];
  sections: Section[];
  notes: string;
  passwordHistory: { replacedAt: Timestamp }[];
  attachments: { id: Uuid; name: string; size: number; mime: string; addedAt: Timestamp }[];
  passkeys: { rpId: string; userName: string; createdAt: Timestamp }[];
  createdAt: Timestamp;
  updatedAt: Timestamp;
  revision: number;
}

export interface Field {
  id: string;
  label: string;
  type: FieldType;
  value: string;
  section?: string | null;
}

export interface ItemInput {
  kind: ItemKind;
  title: string;
  urls: string[];
  tags: string[];
  favorite: boolean;
  icon: string | null;
  fields: Field[];
  sections: Section[];
  notes: string;
}

/** Full item as returned by `item_get_for_edit` (concealed values included). */
export interface ItemFull {
  id: Uuid;
  vaultId: Uuid;
  kind: ItemKind;
  overview: {
    title: string;
    subtitle: string;
    urls: string[];
    tags: string[];
    favorite: boolean;
    archived: boolean;
    trashedAt?: Timestamp | null;
    icon?: string | null;
  };
  details: {
    fields: Field[];
    sections: Section[];
    notes: string;
  };
  createdAt: Timestamp;
  updatedAt: Timestamp;
  revision: number;
}

export interface VersionView {
  revision: number;
  savedAt: Timestamp;
  item: ItemView;
}

export interface CopyResult {
  clearsIn: number;
}

export interface TotpCode {
  code: string;
  period: number;
  remaining: number;
  issuer: string | null;
  account: string | null;
}

export interface Strength {
  score: number;
  label: string;
  guessesLog10: number;
  crackTime: string;
  warning: string | null;
  suggestions: string[];
}

export type Recipe =
  | {
      mode: "characters";
      length: number;
      lowercase: boolean;
      uppercase: boolean;
      digits: boolean;
      symbols: boolean;
      avoidAmbiguous: boolean;
      exclude: string;
    }
  | { mode: "passphrase"; words: number; separator: string; capitalize: boolean; includeNumber: boolean; language: "pt" | "en" }
  | { mode: "pin"; length: number };

export interface Generated {
  value: string;
  entropyBits: number;
  strength: Strength;
}

export interface SecurityStatus {
  recoveryEnabled: boolean;
  recoveryCreatedAt: Timestamp | null;
  passwordChangedAt: Timestamp | null;
  createdAt: Timestamp | null;
  deviceKeys: string[];
}

export interface HealthItemRef {
  id: Uuid;
  title: string;
  kind: ItemKind;
  subtitle: string;
  urls: string[];
}

export interface HealthFinding {
  item: HealthItemRef;
  detail: string;
  field: string;
  days: number | null;
}

export interface HealthReport {
  checkedAt: Timestamp;
  totalItems: number;
  passwords: number;
  weak: HealthFinding[];
  reused: { items: HealthItemRef[] }[];
  insecureSites: HealthFinding[];
  expiring: HealthFinding[];
  old: HealthFinding[];
}

export interface BreachReport {
  checked: number;
  hits: { itemId: Uuid; title: string; field: string; count: number }[];
}

export type ImportSource = "auto" | "chrome" | "firefox" | "bitwarden" | "onePassword" | "lastPass" | "protonPass" | "keePass" | "genericCsv";

export interface ImportPreview {
  fileName: string;
  source: string;
  total: number;
  duplicates: number;
  withoutPassword: number;
  folders: string[];
  skipped: [string, string][];
  rows: { index: number; title: string; subtitle: string; kind: ItemKind; folder: string | null; issue: string | null; duplicate: boolean }[];
}

export interface ExportResult {
  path: string;
  count: number;
  cloudSynced: boolean;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string;
  date: string | null;
  critical: boolean;
}

export interface CloudStatus {
  connected: boolean;
  email: string | null;
  server: string;
  lastSyncAt: number;
  lastError: string | null;
  pending: number;
  syncing: boolean;
}

export interface SyncOutcome {
  pushed: number;
  pulled: number;
  conflictsResolved: number;
  rejected: number;
}

export interface CloudDevice {
  id: Uuid;
  name: string;
  platform: string;
  createdAt: number;
  lastSeenAt: number;
  current: boolean;
}

export interface CloudMe {
  email: string;
  plan: string;
  twoFactor: boolean;
  recoveryCodesLeft: number;
  createdAt: number;
  devices: CloudDevice[];
  items: number;
}
