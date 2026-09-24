# Como o Mocó protege o que você guarda

Este documento explica, sem marketing, o que o Mocó faz para proteger seus dados, o que ele
consegue e o que ele não consegue ver, e o que acontece em cada situação ruim. As decisões
técnicas e suas justificativas estão em [`DECISIONS.md`](../DECISIONS.md).

## Em uma frase

Tudo o que você guarda é cifrado no seu computador, com uma chave que nasce da sua senha
mestra **e** de uma Chave Secreta aleatória que só existe nos seus dispositivos. O servidor
guarda apenas dados já cifrados e não tem como abri-los.

## As duas chaves que abrem o seu Mocó

1. **Senha mestra** — a única que você decora. Nunca é armazenada nem enviada a lugar nenhum.
2. **Chave Secreta** — 128 bits aleatórios gerados no seu computador na criação da conta
   (formato `M1-XXXXXX-…`). Fica guardada no Windows, protegida pela sua conta de usuário, e
   impressa no seu Kit de Emergência. Nunca vai para o servidor.

A partir delas, o Mocó calcula (Argon2id com 64 MiB de memória, depois HKDF-SHA256) a chave
que desembrulha a **Chave da Conta**, que por sua vez abre as chaves de cada cofre.

**Por que duas?** Se alguém roubar o banco de dados do servidor ou uma cópia do arquivo do
seu cofre, não basta adivinhar a sua senha: seria preciso adivinhar também 128 bits
aleatórios — impraticável, mesmo que a sua senha seja fraca.

**Limite honesto:** a Chave Secreta fica no mesmo computador, protegida pelo Windows. Quem
tiver o seu computador **e** a sua senha do Windows enfrenta só a senha mestra (ainda
protegida pelo Argon2id). Por isso a senha mestra continua importando.

## O que é cifrado

Tudo que você digita: nomes dos itens, sites, usuários, senhas, notas, etiquetas, anexos,
nomes dos cofres, histórico de versões. Cada item é cifrado com XChaCha20-Poly1305 e leva um
cabeçalho autenticado que diz a qual conta, cofre, item e versão ele pertence — se alguém
trocar, mover, duplicar ou "voltar no tempo" um item, o Mocó percebe e recusa.

Os tamanhos são arredondados em faixas para não revelar o comprimento exato do conteúdo.

## O que o servidor do Mocó consegue ver

Quando a sincronização está ativa, o servidor conhece:
- seu e-mail (para você entrar), a data de criação da conta e seu plano;
- a lista de dispositivos conectados (nome do computador, sistema, datas de uso);
- quantos itens você tem, identificadores aleatórios, datas de alteração e tamanhos aproximados;
- registros de acesso (entradas, troca de senha, dispositivos removidos).

Se você compartilha cofres, ele também sabe com quem (contas e papéis) e guarda os convites
cifrados. Para achar alguém pelo e-mail, o servidor informa se aquele e-mail tem conta no
Mocó — essa busca exige estar logado e tem limite de tentativas.

Quem só pode **ver** um cofre compartilhado recebe a chave para ler; a proibição de alterar
é aplicada pelo servidor. Ao remover alguém, a chave do cofre é trocada: a pessoa não lê nada
escrito depois — mas o que ela já viu pode ter sido copiado.

O servidor **não** consegue ver: nomes dos itens, sites, usuários, senhas, notas, anexos,
etiquetas ou nomes de cofres. Ele também não recebe nada que permita testar senhas — o
login é feito com uma assinatura digital de uso único, derivada da senha e da Chave Secreta.

## No seu computador

- **Área de transferência:** senhas copiadas não entram no histórico do Windows (Win+V) nem
  na área de transferência na nuvem, e são apagadas depois de 90 s (configurável) — só se
  você não copiou outra coisa nesse meio-tempo.
- **Tranca automática:** por inatividade, ao bloquear o Windows, ao suspender, ou ao
  minimizar (opcional). Trancar apaga as chaves da memória e recarrega a interface.
- **Capturas de tela:** a janela do Mocó é invisível para prints, gravações e
  compartilhamento de tela (configurável).
- **Windows Hello:** opcional. Usa uma chave guardada no chip de segurança (TPM) do
  computador. A senha mestra continua sendo pedida periodicamente, para você não esquecê-la.
- **Tentativas erradas** atrasam novas tentativas progressivamente.

## E se…

| Situação | O que acontece |
|---|---|
| Esqueci a senha mestra | Com o **Código de Recuperação** (folha separada) + a Chave Secreta, você define uma senha nova. Sem o código, não há como recuperar — nem por nós. |
| Perdi a Chave Secreta, mas tenho o computador | Ela está guardada nele: veja em Configurações › Kit de Emergência e imprima de novo. |
| Troquei de computador | Instale o Mocó, escolha "Já uso o Mocó em outro computador" e use e-mail, senha mestra e Chave Secreta (do Kit). |
| Perdi ou roubaram um computador | Em outro dispositivo: Configurações › Sincronização › remova o dispositivo e troque a senha mestra. O que já estava nele continua cifrado. |
| Perdi o celular do autenticador (2FA) | Use um dos códigos de emergência entregues ao ativar a verificação em duas etapas. |
| O servidor do Mocó vazou | Os invasores levam dados cifrados que não conseguem abrir, porque não têm sua Chave Secreta. |
| O servidor tentar me enganar | Itens adulterados, trocados ou antigos são recusados pelo aplicativo. |
| O servidor tentar se passar por quem compartilha comigo | O Mocó guarda a chave de cada pessoa na primeira vez que a vê. Se ela mudar, o compartilhamento para até você confirmar — de preferência comparando o número de segurança com a pessoa. |

## Atualizações

O Mocó só instala atualizações assinadas com a chave privada do projeto; a assinatura é
conferida antes de qualquer coisa ser executada. Atualizações de segurança aparecem como
tal; as demais nunca interrompem você.

## Telemetria

Não há telemetria de uso nem anúncios. Saem do seu computador apenas: a sincronização (se
ativada, só dados cifrados), a busca de atualizações (só a versão do app) e, se você pedir, a
verificação de vazamentos (anônima, por k-anonimato: só 5 caracteres do hash SHA-1 da senha).

## Primitivas usadas

Argon2id (RFC 9106), HKDF-SHA256 (RFC 5869), XChaCha20-Poly1305 com compromisso de chave,
X25519 e Ed25519 (dalek), aleatoriedade do sistema operacional. Nada de criptografia
inventada. Bibliotecas RustCrypto e dalek, com vetores de teste oficiais na suíte de testes.

## Reportar um problema de segurança

Abra uma issue privada ("Security advisory") no repositório do GitHub. Por favor, não
publique detalhes antes de conversarmos.
