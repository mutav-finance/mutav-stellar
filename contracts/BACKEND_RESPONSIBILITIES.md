# Backend Responsibilities — O que fica fora dos contratos Soroban

Esta arquitetura é híbrida por design: Soroban gerencia estado e lógica financeira;
Stellar Classic e a API Etherfuse movimentam valor real. Tudo abaixo é responsabilidade
do backend — nunca deve entrar em contratos Soroban.

> **Princípio central:** o contrato Fund não retém USDC de forma permanente.
> Todo valor do fundo vive como TESOURO na Classic wallet. O contrato é puramente
> um livro contábil (AUM, supply, NAV, fila de resgate).

---

## On-ramp — mensalidade de imobiliária (BRL → USDC → TESOURO)

1. **POST /quote + POST /order (Etherfuse API)**
   - Gera cotação BRL→USDC e cria ordem de pagamento
   - Retorna instruções PIX para a imobiliária
   - Auth: API key no header sem prefixo `Bearer`

2. **Detectar confirmação de pagamento**
   - Monitorar webhook ou polling da Etherfuse
   - Quando USDC chegar na Classic wallet do backend, acionar próximo passo

3. **Chamar `receive_payment()` no contrato Fund**
   - Envia USDC ao contrato e dispara o split 20/80 on-chain
   - O contrato envia 20% ao protocol wallet e 80% à Classic wallet

4. **Converter 80% USDC → TESOURO via Etherfuse eYield**
   - Transação Stellar Classic da Classic wallet
   - TESOURO fica na Classic wallet gerando yield (~13% APY)

## On-ramp — depósito de investidor (USDC → TESOURO)

5. **Investidor chama `deposit_investor()` no contrato Fund**
   - O contrato puxa o USDC do investidor e repassa 100% à Classic wallet imediatamente
   - MUTAV é mintado ao NAV corrente

6. **Converter USDC do investidor → TESOURO via Etherfuse eYield**
   - Backend monitora o evento `deposit` e dispara a conversão na Classic wallet

---

## Yield (TESOURO → AUM)

7. **Monitorar rendimento do TESOURO periodicamente**
   - Calcular acréscimo de valor em USDC equivalente
   - Chamar `add_yield(amount_usdc)` no contrato Fund para atualizar o NAV

---

## Off-ramp — resgate de investidor (TESOURO → USDC → investidor)

> **Restrição crítica:** Soroban não suporta memo. O off-ramp da Etherfuse exige memo
> obrigatório em transações Stellar Classic. Por isso o off-ramp NUNCA pode sair
> diretamente de um contrato Soroban — sem memo = "orphan order".

8. **Chamar `process_redemptions()` semanalmente**
   - Contrato processa fila FIFO até o cap de 2.5% do AUM
   - Burns MUTAV ao NAV da data de execução (não da solicitação)
   - Retorna total de USDC que o backend precisa providenciar

9. **Enviar TESOURO via Stellar Classic com MEMO obrigatório**
   - Quantidade equivalente ao retorno de `process_redemptions()`
   - Transação parte da Classic wallet (não do contrato Soroban)
   - MEMO deve conter o identificador de ordem da Etherfuse

10. **Etherfuse processa → USDC na Classic wallet**
    - Backend deposita o USDC recebido no contrato Fund (via token transfer)

11. **Chamar `fulfill_redemption(investor)` para cada investidor processado**
    - Contrato transfere USDC ao investidor
    - Monitorar evento `rdy_rdmpt` para saber quais investidores chamar

## Off-ramp — cobertura de inadimplência (TESOURO → PIX → landlord)

12. **Detectar inadimplência no sistema off-chain**

13. **Enviar TESOURO via Stellar Classic com MEMO obrigatório → PIX ao landlord**

14. **Chamar `cover_default(amount_usdc, destination)` no contrato**
    - Puramente contábil: decrementa AUM e registra o endereço do landlord on-chain para auditoria
    - Nenhum USDC passa pelo contrato

## Taxas de gestão

15. **Mensalmente: pagar 1% do AUM em TESOURO/USDC da Classic wallet ao protocol wallet**

16. **Chamar `charge_mgmt_fee()` no contrato Fund**
    - Puramente contábil: decrementa AUM e registra o timestamp
    - Impede double-charge no período de 30 dias

---

## Manutenção do estado Soroban

17. **Renovar TTL do contrato periodicamente (cron job)**
    - Chamar `extend_ttl()` no contrato Fund antes do estado expirar
    - Recomendado: executar a cada ~25 dias (TTL configurado para ~30 dias)
    - Sem isso o estado do contrato expira e o fundo fica inacessível

---

## Ambiente de testes

A Etherfuse disponibiliza devnet para testes completos sem dinheiro real.
Configurar variável de ambiente `ETHERFUSE_ENV=devnet` no backend antes de integrar
com a testnet da Stellar.
