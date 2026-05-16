# Introdução ao Contrato SGR Fund

O contrato é o **coração financeiro do SGR**. Ele gerencia um fundo de investimento que usa imóveis alugados como base e o Tesouro Direto brasileiro (via Etherfuse) como reserva. Pense nele como um cofre inteligente com regras automáticas gravadas em código na blockchain Stellar.

---

## A unidade de participação: o token MUTAV

Quem investe no fundo recebe **tokens MUTAV** em troca. O MUTAV representa sua fatia do fundo — quanto mais o fundo rende, mais vale cada MUTAV. É como uma cota de fundo imobiliário, mas digital e rastreável na blockchain.

---

## Quem manda no quê

O contrato tem dois perfis de controle com permissões diferentes:

- **Dono (carteira fria):** É o "sócio-controlador". Cuida de decisões importantes e irreversíveis — trocar quem são os responsáveis, registrar inadimplências, configurar novos contratos. Usa uma carteira guardada em local seguro, sem acesso frequente.

- **Operador (carteira quente):** É o "gerente do dia a dia". Executa operações rotineiras — receber pagamentos, processar saques, registrar rendimentos. Usa uma carteira de acesso mais fácil, mas com poderes limitados.

Separar os dois evita que um ataque à carteira operacional dê controle total do fundo.

---

## Funcionalidades

### 1. Configuração inicial

Quando o contrato é publicado na blockchain, ele precisa ser configurado uma única vez: quem é o dono, quem é o operador, qual moeda aceita (USDC), para onde vai o dinheiro do fundo, e uma série de parâmetros financeiros como:

- Quanto pode sair por semana (limite de resgates)
- Qual é a taxa de gestão mensal
- Qual é a taxa cobrada no resgate
- Qual a fatia que vai para o protocolo em cada pagamento recebido

Depois de configurado, esses parâmetros não mudam.

---

### 2. Receber pagamento de uma imobiliária

Quando uma imobiliária paga o aluguel garantido, o operador registra esse pagamento no contrato. O valor entra e é dividido automaticamente:

- Uma fatia pequena vai para o protocolo (a empresa que mantém o sistema)
- O restante vai para a carteira do fundo, que será convertido em Tesouro Direto via Etherfuse

O patrimônio total do fundo (AUM) sobe na proporção do que ficou para o fundo.

**Como os números mudam:**

| | Antes | Depois |
|---|---|---|
| Pagamento recebido | — | 100.000 USDC |
| Fatia do protocolo (20%) | — | − 20.000 USDC |
| AUM | 1.000.000 USDC | 1.080.000 USDC |
| Supply (tokens) | 1.000.000 MUTAV | 1.000.000 MUTAV |
| NAV (AUM ÷ Supply) | 1,00 USDC | **1,08 USDC** |

O supply não muda — nenhum token novo é criado. Só o AUM sobe, então cada MUTAV existente passa a valer mais.

---

### 3. Investidor deposita dinheiro

Um investidor envia USDC e em troca recebe tokens MUTAV. A quantidade de MUTAV que ele recebe depende do valor atual do fundo — se o fundo já rendeu, cada MUTAV vale mais, então ele recebe menos tokens pelo mesmo USDC (mas cada token vale mais). O USDC depositado vai direto para a carteira do fundo, que o converte em Tesouro Direto. O contrato não guarda USDC em si.

**Como os números mudam:**

> Fórmula: tokens recebidos = USDC depositado × supply atual ÷ AUM atual

| | Antes | Depois |
|---|---|---|
| Depósito do investidor | — | 100.000 USDC |
| AUM | 1.080.000 USDC | 1.180.000 USDC |
| Supply (tokens) | 1.000.000 MUTAV | 1.092.592 MUTAV |
| NAV (AUM ÷ Supply) | 1,08 USDC | **1,08 USDC** |

O investidor recebeu 92.592 MUTAV (menos do que depositou em USDC), porque cada MUTAV já valia 1,08. O NAV não muda — o depósito é proporcional ao patrimônio atual, então não dilui nem valoriza os demais investidores.

---

### 4. Solicitar resgate

O investidor que quer sair do fundo solicita o resgate dos seus tokens MUTAV. Esses tokens são **bloqueados** imediatamente (saem do saldo disponível dele) e entram numa fila de espera. O preço de saída **não é calculado agora** — será calculado quando o operador processar a fila, garantindo que todos saiam pelo valor justo do dia da execução.

**Como os números mudam no momento do pedido:**

Nenhuma mudança em AUM ou NAV ainda. Os tokens apenas saem do saldo disponível do investidor e ficam bloqueados.

**Como os números mudam quando o operador processa a fila:**

> Fórmula: USDC a receber = MUTAV resgatado × AUM atual ÷ supply atual

| | Antes | Depois |
|---|---|---|
| MUTAV resgatado | — | 100.000 MUTAV |
| AUM | 1.080.000 USDC | 972.000 USDC |
| Supply (tokens) | 1.000.000 MUTAV | 900.000 MUTAV |
| NAV (AUM ÷ Supply) | 1,08 USDC | **1,08 USDC** |

O investidor receberá 108.000 USDC (100.000 × 1,08). AUM e supply caem proporcionalmente, então o NAV dos demais investidores permanece igual.

---

### 5. Cancelar pedido de resgate

Se o investidor mudou de ideia antes de o operador processar a fila, ele pode cancelar. Os tokens bloqueados voltam para o saldo dele instantaneamente, sem mexer na fila dos outros.

---

### 6. Processar a fila de resgates

O operador executa essa função periodicamente (normalmente uma vez por semana). O contrato:

1. Verifica quanto do patrimônio pode sair naquela semana (limite de 2,5%, por exemplo)
2. Atende os investidores em ordem de chegada até o limite ser atingido
3. Calcula o valor em USDC de cada pedido pelo NAV do dia
4. Queima os tokens MUTAV correspondentes
5. Registra quanto USDC precisa ser buscado no Etherfuse para pagar cada um
6. Define um prazo para o pagamento acontecer

Quem não couber no limite da semana fica na fila para a próxima rodada.

---

### 7. Pagar o investidor

Após o operador buscar o USDC no Etherfuse e depositá-lo no contrato, ele aciona essa função para cada investidor na fila de pagamento. O contrato deduz uma pequena taxa de resgate e envia o restante diretamente para a carteira do investidor. Se o prazo vencer sem que o operador pague, o investidor tem um caminho alternativo de proteção (ver abaixo).

---

### 8. Resgatar após prazo vencido (proteção do investidor)

Se o operador não pagou dentro do prazo configurado, o investidor pode acionar esse mecanismo de segurança por conta própria. Seus tokens MUTAV são **restaurados** como se o resgate nunca tivesse acontecido, e o patrimônio do fundo é corrigido de volta. Isso protege o investidor de ficar preso sem receber nem ter seus tokens de volta caso o backend falhe.

---

### 9. Registrar rendimento do Tesouro

O operador registra periodicamente o rendimento que chegou via Etherfuse (juros do Tesouro Direto). Isso aumenta o patrimônio do fundo e, consequentemente, o NAV — ou seja, cada MUTAV passa a valer mais. Existe um limite por chamada (por exemplo, máximo de 5% do patrimônio atual de uma vez) para evitar manipulação.

**Como os números mudam:**

| | Antes | Depois |
|---|---|---|
| Rendimento registrado | — | + 10.000 USDC |
| AUM | 1.000.000 USDC | 1.010.000 USDC |
| Supply (tokens) | 1.000.000 MUTAV | 1.000.000 MUTAV |
| NAV (AUM ÷ Supply) | 1,00 USDC | **1,01 USDC** |

O supply não muda — nenhum token novo é criado. O rendimento puro aumenta o AUM e valoriza cada MUTAV existente.

---

### 10. Registrar taxa de aluguel recebida

Funciona igual ao registro de rendimento, mas especificamente para taxas administrativas dos imóveis (como taxa de gestão imobiliária paga pelos locatários). Também tem limite por chamada.

---

### 11. Cobrar taxa de gestão mensal

Uma vez por mês (com intervalo mínimo de 30 dias), o operador cobra a taxa de gestão do fundo. Isso reduz o patrimônio — o AUM encolhe um pouco, o que faz o NAV cair levemente. O pagamento real acontece fora da blockchain, pelo Etherfuse; o contrato apenas registra o desconto contábil.

**Como os números mudam:**

> Fórmula: taxa = AUM × taxa_gestão_bps ÷ 10.000 (ex: 100 bps = 1%)

| | Antes | Depois |
|---|---|---|
| Taxa de gestão cobrada (1%) | — | − 10.000 USDC |
| AUM | 1.000.000 USDC | 990.000 USDC |
| Supply (tokens) | 1.000.000 MUTAV | 1.000.000 MUTAV |
| NAV (AUM ÷ Supply) | 1,00 USDC | **0,99 USDC** |

O supply não muda. Só o AUM cai, então cada MUTAV passa a valer um pouco menos — o custo da gestão é distribuído proporcionalmente entre todos os investidores.

---

### 12. Registrar pagamento fora da blockchain

Quando o fundo precisa pagar algo diretamente (como repassar renda de aluguel ao proprietário via PIX), isso não pode ser feito pelo contrato Soroban — a Stellar Clássica usa um campo especial de memo que contratos Soroban não conseguem enviar. Então o operador registra esse pagamento aqui para fins de auditoria: o patrimônio baixa, e o endereço de destino fica gravado na blockchain.

**Como os números mudam:**

| | Antes | Depois |
|---|---|---|
| Pagamento off-chain registrado | — | − 50.000 USDC |
| AUM | 1.000.000 USDC | 950.000 USDC |
| Supply (tokens) | 1.000.000 MUTAV | 1.000.000 MUTAV |
| NAV (AUM ÷ Supply) | 1,00 USDC | **0,95 USDC** |

Comportamento idêntico à taxa de gestão: só o AUM cai, o supply fica igual, e o NAV reflete a saída real de recursos do fundo.

---

### 13. Cobrir inadimplência

Se um inquilino não pagou e o fundo precisa cobrir a garantia ao proprietário, o **dono** (não o operador) aciona essa função. O patrimônio é reduzido no valor correspondente, e o endereço do destinatário fica registrado para rastreabilidade. O pagamento real sai da carteira clássica via Etherfuse.

**Como os números mudam:**

| | Antes | Depois |
|---|---|---|
| Cobertura de inadimplência | — | − 30.000 USDC |
| AUM | 1.000.000 USDC | 970.000 USDC |
| Supply (tokens) | 1.000.000 MUTAV | 1.000.000 MUTAV |
| NAV (AUM ÷ Supply) | 1,00 USDC | **0,97 USDC** |

A inadimplência é um prejuízo real do fundo: o AUM cai e o NAV de todos os investidores cai junto. É por isso que a função exige o dono (carteira fria) — é uma decisão de alto impacto.

---

### 14. Transferência de propriedade do contrato

Para evitar que um erro de digitação transfira o controle do fundo para um endereço inexistente (bloqueando tudo permanentemente), a troca de dono funciona em **dois passos**:

1. O dono atual nomeia o novo endereço
2. O novo endereço precisa **confirmar** que aceita o controle

Só após a confirmação o controle muda. Se o endereço indicado for errado, basta nomear outro.

---

### 15. Trocar o operador

O dono pode substituir o operador a qualquer momento (por exemplo, em caso de comprometimento da carteira quente). O novo endereço passa a ter as permissões operacionais imediatamente.

---

### 16. Operações com o token MUTAV

O MUTAV segue o padrão de tokens da Stellar (SEP-0041), o que significa que funciona como qualquer outro token da rede:

- **Transferir:** Mandar MUTAV de uma carteira para outra
- **Aprovar gasto:** Autorizar outra carteira a gastar seus tokens (para uso em outros contratos ou plataformas)
- **Queimar:** Destruir tokens diretamente — o patrimônio é reduzido proporcionalmente para que o NAV dos demais investidores não mude
- **Consultar saldo e decimais:** Funções padrão de leitura

---

### 17. Consultas públicas

O contrato disponibiliza diversas informações que qualquer pessoa pode consultar sem custo:

| Consulta | O que retorna |
|---|---|
| NAV | Valor atual de cada token MUTAV em USDC |
| AUM | Patrimônio total do fundo |
| Supply total | Quantos tokens MUTAV existem |
| Saldo de um endereço | Quantos MUTAV uma carteira específica tem |
| Pedido pendente | Quanto MUTAV um investidor tem aguardando processamento |
| Pronto para resgate | Quanto USDC um investidor tem a receber |
| Disponível esta semana | Quanto ainda pode ser resgatado no ciclo atual |
| Parâmetros | Taxas, limites e janela de pagamento configurados |

---

### 18. Manutenção de dados na blockchain

Na Stellar, dados armazenados on-chain expiram se não forem renovados periodicamente. O contrato tem funções para isso:

- O operador renova os dados globais do fundo a cada ~25 dias
- Qualquer pessoa pode renovar o registro de saldo de um investidor específico — útil para quem fica muito tempo sem movimentar a carteira

---

## O fluxo completo em uma frase

Uma imobiliária paga o aluguel → o fundo recebe e investe em Tesouro → os tokens MUTAV se valorizam → o investidor solicita resgate → o fundo processa na semana, calcula o valor justo, converte o Tesouro de volta em USDC → e paga o investidor, descontando uma pequena taxa.
