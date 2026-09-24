// Item kinds: fields, labels and behaviors. The core only knows canonical field ids
// ("username", "password", "number"…); everything a person reads lives here.

import type { FieldType, ItemKind } from "./types";

export type Glaze = "cobalt" | "sky" | "moss" | "ochre" | "clay" | "plum" | "slate" | "ink";

export interface FieldTemplate {
  id: string;
  label: string;
  type: FieldType;
  placeholder?: string;
  options?: string[];
  /** Offer the password generator on this field. */
  generate?: boolean;
  /** Visually large in the item view (e.g. the card number, the Wi-Fi password). */
  prominent?: boolean;
  /** Short helper under the input in the editor. */
  hint?: string;
  section?: string;
}

export interface KindTemplate {
  kind: ItemKind;
  label: string;
  plural: string;
  /** One-line description shown in the "Novo item" picker. */
  blurb: string;
  glaze: Glaze;
  titlePlaceholder: string;
  /** Websites list (logins). */
  urls?: boolean;
  fields: FieldTemplate[];
  sections?: { id: string; label: string }[];
  /** Search aliases so "senha do wifi", "cartão", "rg" find the right things. */
  aliases: string[];
}

const UF = ["AC", "AL", "AP", "AM", "BA", "CE", "DF", "ES", "GO", "MA", "MT", "MS", "MG", "PA", "PB", "PR", "PE", "PI", "RJ", "RN", "RS", "RO", "RR", "SC", "SP", "SE", "TO"];

export const TEMPLATES: KindTemplate[] = [
  {
    kind: "login",
    label: "Login",
    plural: "Logins",
    blurb: "Usuário e senha de um site ou app",
    glaze: "cobalt",
    titlePlaceholder: "Ex.: Nubank, Gmail, Netflix",
    urls: true,
    fields: [
      { id: "username", label: "Usuário ou e-mail", type: "text", placeholder: "voce@email.com" },
      { id: "password", label: "Senha", type: "concealed", generate: true },
      {
        id: "totp",
        label: "Código de verificação (2FA)",
        type: "totp",
        placeholder: "Chave ou link otpauth://",
        hint: "Cole a chave que o site mostra ao ativar a verificação em duas etapas.",
      },
    ],
    aliases: ["login", "conta", "site", "senha", "acesso"],
  },
  {
    kind: "password",
    label: "Senha",
    plural: "Senhas",
    blurb: "Só uma senha, sem usuário",
    glaze: "slate",
    titlePlaceholder: "Ex.: Senha do cofre de casa",
    fields: [{ id: "password", label: "Senha", type: "concealed", generate: true, prominent: true }],
    aliases: ["senha", "password", "pin"],
  },
  {
    kind: "card",
    label: "Cartão",
    plural: "Cartões",
    blurb: "Crédito, débito, pré-pago ou vale",
    glaze: "clay",
    titlePlaceholder: "Ex.: Nubank Ultravioleta",
    fields: [
      { id: "cardholder", label: "Nome impresso", type: "text", placeholder: "ANA M SILVA" },
      { id: "number", label: "Número", type: "cardNumber", prominent: true, placeholder: "0000 0000 0000 0000" },
      { id: "expiry", label: "Validade", type: "monthYear", placeholder: "MM/AA" },
      { id: "cvv", label: "Código de segurança", type: "pin", placeholder: "CVV" },
      { id: "pin", label: "Senha do cartão", type: "pin" },
      { id: "cardType", label: "Tipo", type: "select", options: ["Crédito", "Débito", "Múltiplo", "Pré-pago", "Vale-refeição", "Vale-alimentação"] },
      { id: "issuer", label: "Banco emissor", type: "text", placeholder: "Ex.: Itaú" },
      { id: "phone", label: "Central de atendimento", type: "phone", placeholder: "0800…" },
    ],
    aliases: ["cartao", "cartão", "credito", "débito", "card", "visa", "mastercard", "elo"],
  },
  {
    kind: "identity",
    label: "Identidade",
    plural: "Identidades",
    blurb: "Seus dados para preencher cadastros",
    glaze: "moss",
    titlePlaceholder: "Ex.: Meus dados",
    sections: [
      { id: "personal", label: "Pessoal" },
      { id: "address", label: "Endereço" },
    ],
    fields: [
      { id: "fullName", label: "Nome completo", type: "text", section: "personal" },
      { id: "socialName", label: "Nome social", type: "text", section: "personal" },
      { id: "birthDate", label: "Nascimento", type: "date", section: "personal" },
      { id: "cpf", label: "CPF", type: "cpf", section: "personal", placeholder: "000.000.000-00" },
      { id: "rg", label: "RG", type: "text", section: "personal" },
      { id: "email", label: "E-mail", type: "email", section: "personal" },
      { id: "phone", label: "Celular", type: "phone", section: "personal", placeholder: "(11) 90000-0000" },
      { id: "cep", label: "CEP", type: "cep", section: "address", placeholder: "00000-000" },
      { id: "street", label: "Rua", type: "text", section: "address" },
      { id: "number", label: "Número", type: "text", section: "address" },
      { id: "complement", label: "Complemento", type: "text", section: "address" },
      { id: "district", label: "Bairro", type: "text", section: "address" },
      { id: "city", label: "Cidade", type: "text", section: "address" },
      { id: "state", label: "Estado", type: "select", options: UF, section: "address" },
    ],
    aliases: ["identidade", "dados", "cadastro", "endereço", "cpf", "rg", "perfil"],
  },
  {
    kind: "document",
    label: "Documento",
    plural: "Documentos",
    blurb: "RG, CNH, passaporte, título de eleitor…",
    glaze: "ochre",
    titlePlaceholder: "Ex.: CNH da Ana",
    fields: [
      {
        id: "docType",
        label: "Documento",
        type: "select",
        options: ["RG", "CPF", "CNH", "Passaporte", "Título de eleitor", "CTPS", "Certidão", "Reservista", "RNE / CRNM", "Carteira profissional", "Outro"],
      },
      { id: "number", label: "Número", type: "text", prominent: true },
      { id: "fullName", label: "Nome no documento", type: "text" },
      { id: "issuer", label: "Órgão emissor", type: "text", placeholder: "Ex.: SSP/SP, DETRAN-RJ" },
      { id: "issueDate", label: "Emissão", type: "date" },
      { id: "expiryDate", label: "Validade", type: "date" },
      { id: "category", label: "Categoria", type: "text", placeholder: "Ex.: AB (CNH)" },
    ],
    aliases: ["documento", "doc", "rg", "cnh", "passaporte", "titulo", "título", "habilitação"],
  },
  {
    kind: "note",
    label: "Nota segura",
    plural: "Notas seguras",
    blurb: "Texto livre que merece mais que um bloco de notas",
    glaze: "ochre",
    titlePlaceholder: "Ex.: Combinação do cadeado da academia",
    fields: [],
    aliases: ["nota", "anotação", "texto", "note"],
  },
  {
    kind: "wifi",
    label: "Wi-Fi",
    plural: "Redes Wi-Fi",
    blurb: "Nome e senha da rede, com QR code para visitas",
    glaze: "sky",
    titlePlaceholder: "Ex.: Wi-Fi de casa",
    fields: [
      { id: "ssid", label: "Nome da rede", type: "text", placeholder: "Ex.: CASA_5G" },
      { id: "password", label: "Senha", type: "concealed", generate: true, prominent: true },
      { id: "security", label: "Segurança", type: "select", options: ["WPA3", "WPA2", "WPA", "WEP", "Aberta"] },
    ],
    aliases: ["wifi", "wi-fi", "rede", "internet", "roteador"],
  },
  {
    kind: "license",
    label: "Licença de software",
    plural: "Licenças",
    blurb: "Chaves de produto e registros de compra",
    glaze: "plum",
    titlePlaceholder: "Ex.: Microsoft Office",
    fields: [
      { id: "product", label: "Produto", type: "text" },
      { id: "version", label: "Versão", type: "text" },
      { id: "licenseKey", label: "Chave de licença", type: "concealed", prominent: true },
      { id: "licensedTo", label: "Registrado para", type: "text" },
      { id: "email", label: "E-mail da compra", type: "email" },
      { id: "orderNumber", label: "Número do pedido", type: "text" },
      { id: "purchaseDate", label: "Data da compra", type: "date" },
      { id: "expiryDate", label: "Validade", type: "date" },
    ],
    aliases: ["licença", "licenca", "serial", "chave", "software", "produto"],
  },
  {
    kind: "bankAccount",
    label: "Conta bancária",
    plural: "Contas bancárias",
    blurb: "Agência, conta e chaves Pix",
    glaze: "moss",
    titlePlaceholder: "Ex.: Conta corrente Itaú",
    fields: [
      { id: "bank", label: "Banco", type: "text", placeholder: "Ex.: 341 · Itaú" },
      { id: "branch", label: "Agência", type: "text" },
      { id: "accountNumber", label: "Conta", type: "text" },
      { id: "accountType", label: "Tipo de conta", type: "select", options: ["Corrente", "Poupança", "Pagamento", "Salário", "Investimento"] },
      { id: "holder", label: "Titular", type: "text" },
      { id: "pixKeys", label: "Chaves Pix", type: "multiline", placeholder: "Uma por linha: CPF, e-mail, celular ou aleatória" },
      { id: "appPassword", label: "Senha do app / internet banking", type: "concealed" },
      { id: "cardPassword", label: "Senha de 4 ou 6 dígitos", type: "pin" },
      { id: "iban", label: "IBAN / SWIFT", type: "text" },
    ],
    aliases: ["banco", "conta", "pix", "agência", "agencia", "bancária"],
  },
  {
    kind: "server",
    label: "Servidor",
    plural: "Servidores",
    blurb: "Acesso SSH, RDP, FTP ou painel",
    glaze: "slate",
    titlePlaceholder: "Ex.: VPS produção",
    urls: true,
    fields: [
      { id: "host", label: "Endereço", type: "text", placeholder: "Ex.: 203.0.113.10 ou vps.exemplo.com" },
      { id: "port", label: "Porta", type: "number", placeholder: "22" },
      { id: "protocol", label: "Protocolo", type: "select", options: ["SSH", "SFTP", "RDP", "FTP", "VNC", "HTTP(S)"] },
      { id: "username", label: "Usuário", type: "text" },
      { id: "password", label: "Senha", type: "concealed", generate: true },
    ],
    aliases: ["servidor", "server", "ssh", "vps", "rdp", "host"],
  },
  {
    kind: "database",
    label: "Banco de dados",
    plural: "Bancos de dados",
    blurb: "Credenciais e string de conexão",
    glaze: "sky",
    titlePlaceholder: "Ex.: Postgres produção",
    fields: [
      { id: "dbType", label: "Tipo", type: "select", options: ["PostgreSQL", "MySQL", "MariaDB", "SQL Server", "Oracle", "MongoDB", "Redis", "SQLite", "Outro"] },
      { id: "host", label: "Servidor", type: "text" },
      { id: "port", label: "Porta", type: "number" },
      { id: "database", label: "Banco", type: "text" },
      { id: "username", label: "Usuário", type: "text" },
      { id: "password", label: "Senha", type: "concealed", generate: true },
      { id: "options", label: "Parâmetros", type: "text", placeholder: "Ex.: sslmode=require" },
    ],
    aliases: ["banco de dados", "database", "db", "postgres", "mysql", "sql"],
  },
  {
    kind: "apiCredential",
    label: "Credencial de API",
    plural: "Credenciais de API",
    blurb: "Chaves, segredos e tokens",
    glaze: "plum",
    titlePlaceholder: "Ex.: Stripe produção",
    fields: [
      { id: "service", label: "Serviço", type: "text" },
      { id: "keyId", label: "ID da chave", type: "text" },
      { id: "apiKey", label: "Chave", type: "concealed", prominent: true },
      { id: "secret", label: "Segredo", type: "concealed" },
      { id: "environment", label: "Ambiente", type: "select", options: ["Produção", "Homologação", "Desenvolvimento", "Teste"] },
      { id: "expiresAt", label: "Expira em", type: "date" },
      { id: "scopes", label: "Permissões", type: "text" },
    ],
    aliases: ["api", "token", "chave", "key", "secret", "segredo"],
  },
  {
    kind: "sshKey",
    label: "Chave SSH",
    plural: "Chaves SSH",
    blurb: "Chave privada, pública e impressão digital",
    glaze: "ink",
    titlePlaceholder: "Ex.: Notebook pessoal",
    fields: [
      { id: "privateKey", label: "Chave privada", type: "sshPrivateKey" },
      { id: "publicKey", label: "Chave pública", type: "multiline" },
      { id: "fingerprint", label: "Impressão digital", type: "text" },
      { id: "keyType", label: "Tipo", type: "select", options: ["Ed25519", "RSA", "ECDSA"] },
      { id: "passphrase", label: "Frase da chave", type: "concealed" },
    ],
    aliases: ["ssh", "chave", "key", "git"],
  },
  {
    kind: "cryptoWallet",
    label: "Carteira cripto",
    plural: "Carteiras cripto",
    blurb: "Frase de recuperação e endereços",
    glaze: "ochre",
    titlePlaceholder: "Ex.: Carteira de Bitcoin",
    fields: [
      { id: "walletName", label: "Carteira", type: "text", placeholder: "Ex.: Ledger, MetaMask" },
      { id: "network", label: "Rede", type: "text", placeholder: "Ex.: Bitcoin, Ethereum" },
      { id: "seedPhrase", label: "Frase de recuperação", type: "seedPhrase", prominent: true },
      { id: "address", label: "Endereço público", type: "text" },
      { id: "pin", label: "PIN", type: "pin" },
    ],
    aliases: ["cripto", "crypto", "bitcoin", "carteira", "seed", "wallet"],
  },
  {
    kind: "healthPlan",
    label: "Plano de saúde",
    plural: "Planos de saúde",
    blurb: "Carteirinha, operadora e Cartão SUS",
    glaze: "moss",
    titlePlaceholder: "Ex.: Unimed da família",
    fields: [
      { id: "provider", label: "Operadora", type: "text" },
      { id: "plan", label: "Plano", type: "text" },
      { id: "memberNumber", label: "Número da carteirinha", type: "text", prominent: true },
      { id: "holder", label: "Beneficiário", type: "text" },
      { id: "validUntil", label: "Validade", type: "date" },
      { id: "cns", label: "Cartão Nacional de Saúde (SUS)", type: "text" },
      { id: "phone", label: "Central de atendimento", type: "phone" },
    ],
    aliases: ["saúde", "saude", "convênio", "convenio", "plano", "sus", "carteirinha"],
  },
  {
    kind: "vehicle",
    label: "Veículo",
    plural: "Veículos",
    blurb: "Placa, RENAVAM, chassi e seguro",
    glaze: "slate",
    titlePlaceholder: "Ex.: Carro da família",
    fields: [
      { id: "model", label: "Modelo", type: "text" },
      { id: "year", label: "Ano", type: "number" },
      { id: "plate", label: "Placa", type: "text", prominent: true, placeholder: "ABC1D23" },
      { id: "renavam", label: "RENAVAM", type: "text" },
      { id: "chassis", label: "Chassi", type: "text" },
      { id: "color", label: "Cor", type: "text" },
      { id: "insurer", label: "Seguradora", type: "text" },
      { id: "policyNumber", label: "Apólice", type: "text" },
    ],
    aliases: ["veículo", "veiculo", "carro", "moto", "placa", "renavam", "seguro"],
  },
  {
    kind: "custom",
    label: "Personalizado",
    plural: "Personalizados",
    blurb: "Monte do seu jeito, campo por campo",
    glaze: "ink",
    titlePlaceholder: "Dê um nome",
    fields: [],
    aliases: ["personalizado", "outro", "custom"],
  },
];

const BY_KIND = new Map(TEMPLATES.map((t) => [t.kind, t]));

export function template(kind: ItemKind): KindTemplate {
  return BY_KIND.get(kind) ?? BY_KIND.get("custom")!;
}

export const FIELD_TYPE_LABELS: Partial<Record<FieldType, string>> = {
  text: "Texto",
  concealed: "Senha / segredo",
  email: "E-mail",
  url: "Site",
  phone: "Telefone",
  date: "Data",
  number: "Número",
  pin: "PIN",
  totp: "Código 2FA",
  multiline: "Texto longo",
  cpf: "CPF",
  cnpj: "CNPJ",
  cep: "CEP",
};

/** Field types offered when adding a custom field. */
export const CUSTOM_FIELD_TYPES: FieldType[] = ["text", "concealed", "email", "url", "phone", "date", "number", "pin", "totp", "multiline", "cpf", "cnpj", "cep"];
