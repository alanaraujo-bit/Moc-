// pt-BR formatting helpers.

const DAY = 864e5;

const dateFmt = new Intl.DateTimeFormat("pt-BR", { day: "numeric", month: "short" });
const dateYearFmt = new Intl.DateTimeFormat("pt-BR", { day: "numeric", month: "short", year: "numeric" });
const fullFmt = new Intl.DateTimeFormat("pt-BR", { dateStyle: "long", timeStyle: "short" });
const timeFmt = new Intl.DateTimeFormat("pt-BR", { hour: "2-digit", minute: "2-digit" });

export function relativeTime(ts: number, now = Date.now()): string {
  const diff = now - ts;
  if (diff < 45_000) return "agora";
  if (diff < 3_600_000) return `há ${Math.round(diff / 60_000)} min`;
  const d = new Date(ts);
  const today = new Date(now);
  today.setHours(0, 0, 0, 0);
  if (ts >= today.getTime()) return `hoje, ${timeFmt.format(d)}`;
  if (ts >= today.getTime() - DAY) return `ontem, ${timeFmt.format(d)}`;
  if (diff < 7 * DAY) return `há ${Math.ceil(diff / DAY)} dias`;
  return d.getFullYear() === today.getFullYear() ? dateFmt.format(d).replace(".", "") : dateYearFmt.format(d).replace(".", "");
}

export function fullDate(ts: number): string {
  return fullFmt.format(new Date(ts));
}

export function ageInDays(ts: number, now = Date.now()): number {
  return Math.floor((now - ts) / DAY);
}

export function plural(n: number, one: string, many: string): string {
  return `${n.toLocaleString("pt-BR")} ${n === 1 ? one : many}`;
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10240 ? 1 : 0).replace(".", ",")} KB`;
  return `${(n / 1024 / 1024).toFixed(1).replace(".", ",")} MB`;
}

const digits = (s: string) => s.replace(/\D/g, "");

export function formatCardNumber(s: string): string {
  const d = digits(s);
  if (/^3[47]/.test(d)) return [d.slice(0, 4), d.slice(4, 10), d.slice(10, 15)].filter(Boolean).join(" ");
  return d.replace(/(.{4})/g, "$1 ").trim();
}

export function formatCpf(s: string): string {
  const d = digits(s).slice(0, 11);
  return d
    .replace(/^(\d{3})(\d)/, "$1.$2")
    .replace(/^(\d{3})\.(\d{3})(\d)/, "$1.$2.$3")
    .replace(/\.(\d{3})(\d{1,2})$/, ".$1-$2");
}

export function formatCnpj(s: string): string {
  const d = digits(s).slice(0, 14);
  return d
    .replace(/^(\d{2})(\d)/, "$1.$2")
    .replace(/^(\d{2})\.(\d{3})(\d)/, "$1.$2.$3")
    .replace(/\.(\d{3})(\d)/, ".$1/$2")
    .replace(/(\d{4})(\d{1,2})$/, "$1-$2");
}

export function formatCep(s: string): string {
  const d = digits(s).slice(0, 8);
  return d.length > 5 ? `${d.slice(0, 5)}-${d.slice(5)}` : d;
}

export function formatPhone(s: string): string {
  const d = digits(s);
  if (d.startsWith("0800") || d.length > 11) return s.trim();
  if (d.length === 11) return `(${d.slice(0, 2)}) ${d.slice(2, 7)}-${d.slice(7)}`;
  if (d.length === 10) return `(${d.slice(0, 2)}) ${d.slice(2, 6)}-${d.slice(6)}`;
  return s.trim();
}

/** CPF check digits (Receita Federal algorithm). */
export function isValidCpf(s: string): boolean {
  const d = digits(s);
  if (d.length !== 11 || /^(\d)\1{10}$/.test(d)) return false;
  const calc = (len: number) => {
    let sum = 0;
    for (let i = 0; i < len; i++) sum += Number(d[i]) * (len + 1 - i);
    const r = (sum * 10) % 11;
    return r === 10 ? 0 : r;
  };
  return calc(9) === Number(d[9]) && calc(10) === Number(d[10]);
}

export function isValidCnpj(s: string): boolean {
  const d = digits(s);
  if (d.length !== 14 || /^(\d)\1{13}$/.test(d)) return false;
  const calc = (len: number) => {
    const weights = len === 12 ? [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2] : [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    const sum = weights.reduce((acc, w, i) => acc + w * Number(d[i]), 0);
    const r = sum % 11;
    return r < 2 ? 0 : 11 - r;
  };
  return calc(12) === Number(d[12]) && calc(13) === Number(d[13]);
}

/** Luhn check for card numbers. */
export function isValidLuhn(s: string): boolean {
  const d = digits(s);
  if (d.length < 12) return false;
  let sum = 0;
  for (let i = 0; i < d.length; i++) {
    let n = Number(d[d.length - 1 - i]);
    if (i % 2 === 1) {
      n *= 2;
      if (n > 9) n -= 9;
    }
    sum += n;
  }
  return sum % 10 === 0;
}

export function formatDateBR(iso: string): string {
  const m = iso.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  return m ? `${m[3]}/${m[2]}/${m[1]}` : iso;
}

export function displayValue(type: string, value: string): string {
  switch (type) {
    case "cardNumber":
      return formatCardNumber(value);
    case "cpf":
      return formatCpf(value);
    case "cnpj":
      return formatCnpj(value);
    case "cep":
      return formatCep(value);
    case "phone":
      return formatPhone(value);
    case "date":
      return formatDateBR(value);
    default:
      return value;
  }
}
