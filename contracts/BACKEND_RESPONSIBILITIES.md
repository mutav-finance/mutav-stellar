# Backend Responsibilities — O que fica fora dos contratos Soroban

Esta arquitetura é híbrida por design: Soroban gerencia estado e lógica financeira;
Stellar Classic e a API Etherfuse movimentam valor real. Tudo abaixo é responsabilidade
do backend — nunca deve entrar em contratos Soroban.

---

## On-ramp (BRL → USDC → TESOURO)

1. **POST /quote + POST /order (Etherfuse API)**
   - Gera cotação BRL→USDC e cria ordem de pagamento
   - Retorna instruções PIX para a imobiliária
   - Auth: API key no header sem prefixo `Bearer`

2. **Detectar confirmação de pagamento**
   - Monitorar webhook ou polling da Etherfuse
   - Quando USDC chegar na Classic wallet do backend, acionar próximo passo

3. **Chamar `receive_payment()` no contrato Fund**
   - Envia USDC ao contrato e dispara o split 20/80 on-chain
   - O contrato envia 80% de volta para a Classic wallet configurada

4. **Converter 80% USDC → TESOURO via Etherfuse eYield**
   - Transação Stellar Classic da Classic wallet do backend
   - TESOURO fica na Classic wallet gerando yield (~13% APY)

---

## Yield (TESOURO → AUM)

5. **Monitorar rendimento do TESOURO periodicamente**
   - Calcular acréscimo de valor em USDC equivalente
   - Chamar `add_yield(amount_usdc)` no contrato Fund para atualizar o NAV

---

## Off-ramp (TESOURO → BRL via PIX)

> **Restrição crítica:** Soroban não suporta memo. O off-ramp da Etherfuse exige memo
> obrigatório em transações Stellar Classic. Por isso o off-ramp NUNCA pode sair
> diretamente de um contrato Soroban — sem memo = "orphan order".

6. **Detectar evento `redemption_requested` ou inadimplência**
   - Monitorar stream de eventos Stellar do contrato Fund

7. **Enviar TESOURO via Stellar Classic com MEMO obrigatório**
   - Transação parte da Classic wallet do backend (não do contrato Soroban)
   - MEMO deve conter o identificador de ordem da Etherfuse

8. **Etherfuse processa → BRL enviado via PIX**
   - Backend aguarda confirmação da Etherfuse

9. **Sincronizar AUM no contrato**
   - Se foi resgate de investidor: chamar `fulfill_redemption(investor)` para pagar USDC e atualizar AUM
   - Se foi cobertura de inadimplência pela Classic wallet: chamar `record_offchain_payout(amount_usdc)` para decrementar AUM

---

## Manutenção do estado Soroban

10. **Renovar TTL do contrato periodicamente (cron job)**
    - Chamar `extend_ttl()` no contrato Fund antes do estado expirar
    - Recomendado: executar a cada ~25 dias (TTL configurado para ~30 dias)
    - Sem isso o estado do contrato expira e o fundo fica inacessível

---

## Ambiente de testes

A Etherfuse disponibiliza devnet para testes completos sem dinheiro real.
Configurar variável de ambiente `ETHERFUSE_ENV=devnet` no backend antes de integrar
com a testnet da Stellar.
