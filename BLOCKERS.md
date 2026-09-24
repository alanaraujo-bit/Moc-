# Bloqueios que dependem de você

Nada aqui interrompe o restante do projeto — cada item foi contornado para seguir em frente.
Quando voltar, estes são os pontos que exigem uma ação sua.

## 1. Certificado de assinatura de código (Authenticode)

**O que falta.** Um certificado de code signing (idealmente EV ou via Azure Trusted Signing)
em nome da empresa/pessoa que publica o Mocó.

**Impacto enquanto isso.** O instalador funciona, mas o Windows SmartScreen mostra
"O Windows protegeu o computador" nas primeiras instalações.

**O que fazer.** Contratar o certificado (Azure Trusted Signing costuma ser o caminho mais
barato e simples em 2026) e adicionar as credenciais como secrets do GitHub. O pipeline de
release já tem o passo de assinatura preparado para receber essas variáveis.

## 2. Provedor de pagamentos (billing)

**O que falta.** Decidir e criar a conta do provedor (Stripe, Pagar.me, Mercado Pago…),
com dados fiscais da empresa.

**Impacto.** Planos, limites e telas de assinatura existem; a cobrança real fica desligada.

## 3. Verificação física do Windows Hello

**O que falta.** Alguém na frente do computador para tocar o sensor/digitar o PIN.

**Impacto.** O fluxo foi testado até o prompt do Windows Hello aparecer; a confirmação
biométrica precisa de você.

## 4. Domínio

**O que falta.** Registrar um domínio (ex.: `moco.app`, `usemoco.com.br`, `moco.com.br`).

**Impacto.** Site e endpoint de atualização usam endereços provisórios (GitHub Releases /
subdomínio da Vercel) até lá.

## 5. Guardar a chave de assinatura das atualizações (ação recomendada)

**O que é.** A chave que assina as atualizações automáticas foi gerada em
`D:\PROJETOS\moco-secrets\` (fora do repositório) e copiada para os secrets do GitHub
(`TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).

**Por que importa.** Se ela se perder, versões já instaladas não aceitam mais atualizações
(o app só instala o que estiver assinado com ela).

**O que fazer.** Copie a pasta `moco-secrets` para um lugar seguro offline (ou guarde no
próprio Mocó, numa nota segura). Nunca commite esses arquivos.
