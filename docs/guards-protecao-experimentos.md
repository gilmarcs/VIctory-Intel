# Guards: proteção de regiões e agrupamento experimental

Registro local de diagnóstico de 26/11/2026, neste clone, em Windows x64 sob
Proton Experimental e Intel Arc B580. Os tempos abaixo são amostras, não
benchmarks reproduzíveis nem prova de correção visual.

**Atualização após revisão:** os dois protótipos de agrupamento foram retirados
do código. As variáveis `VICTORY_GUARD_BATCH_PROTECT` e
`VICTORY_GUARD_BATCH_OPEN` não têm mais efeito. `VirtualQuery` não garante que
as proteções continuem uniformes até a chamada de `VirtualProtect`, que devolve
apenas a proteção anterior da primeira página. O protótipo de proteção também
poderia repetir tentativas de lotes decrescentes sob falha. Os trechos abaixo
descrevem os experimentos históricos, não uma funcionalidade disponível.

## O problema investigado

Em `crates/victory-gpu/src/watch.rs`, os guards usam proteção de memória para
detectar escritas do título. O lock global `GUARDS` também protege os estados
de blocos, aliases e remapeamentos. A proteção de uma faixa é tentada inteira;
se falhar, o caminho original tenta proteger seus blocos individualmente e
trata os buracos encontrados.

Uma execução com perfil detalhado registrou, no intervalo terminado às
17:38:33 UTC (`run-20261126-143505.log`):

- 1.723 chamadas de proteção de regiões, 174 falhas, 2,60 s decorridos no
  conjunto; 1.293 chamadas de proteção de aliases, 0,22 s.
- 1.849 chamadas para abrir blocos, 2,03 s decorridos no conjunto.
- Lock retido por 2,86 s no caminho `protect` e 1,74 s em `write`.
- De 32 falhas amostradas, 25 envolviam múltiplas alocações e 13 incluíam
  memória não comprometida; categorias podem se sobrepor.

As chamadas e os tempos de posse do lock se sobrepõem, não devem ser somados.
Tempos decorridos incluem possível desescalonamento; esperas somadas de várias
threads podem exceder o tempo de quadro. `changing protections` inclui, no
caminho de abertura, processamento além das chamadas `VirtualProtect`. O
`translate --validate --pipelines --jobs 10` estava concorrendo com essa
execução, podendo afetar os tempos. `VICTORY_GUARD_PROFILE=1` adiciona
instrumentação e consultas diagnósticas.

## Protótipo: `VICTORY_GUARD_BATCH_PROTECT=1`

Protótipo removido. Em `protect_here`, **após falhar a tentativa da faixa
inteira**, consulta-se a região e tenta-se proteger em grupo uma sequência de
dois ou mais blocos dentro de uma única região comprometida e uniformemente
`PAGE_READONLY` ou `PAGE_READWRITE`. Região inadequada ou tentativa de grupo
malsucedida mantém o fallback bloco a bloco. O protótipo não altera a abertura
nem desliga a proteção das outras views.

`VirtualQuery` tem custo próprio e não torna impossíveis mudanças posteriores
de mapeamento ou proteção. O protótipo continua experimental: os testes ainda
não estabelecem segurança em todos os remapeamentos concorrentes ou correção
visual no jogo. O mecanismo de proteção e a detecção de escritas por aliases
não devem ser removidos apenas para reduzir as métricas.

### Testes isolados

Na build Windows de testes sob Proton, com o protótipo ligado:

- `failed_range_fallback_protects_committed_run_before_hole` passou: região
  comprometida seguida por buraco.
- Passaram os testes concorrentes de detecção de escritas nas views, inclusive
  através de aliases.
- `what_is_left_alone_is_no_known_hole` falhou; já havia falhado anteriormente
  sob Proton, mas nesta etapa não houve comparação com build anterior.
- `holes_are_known_until_the_runtime_tells` e
  `no_mapping_is_lost_between_askers` falharam com o protótipo tanto ligado
  quanto desligado no mesmo binário. A causa dessas falhas não foi determinada.

Isso não prova que o protótipo seja a causa das falhas nem que seja seguro.

### Observações no jogo

Na execução com o protótipo ligado (`run-20261126-144737.log`), três amostras
recentes levaram 5,1 s, 7,2 s e 5,2 s por quadro. Na execução seguinte, sem a
variável (`run-20261126-145551.log`), três amostras levaram 4,9 s, 8,5 s e
6,0 s. Ambas tinham perfil de guards, `VICTORY_CARD_BUFFERS_MIN_MB=1` e
`VICTORY_ECHO_PROBE=1`; a sonda de eco acrescenta trabalho. A carga dos
quadros e a cena podem variar, e o relatório atual não separa as chamadas
agrupadas das demais.

**Conclusão:** os intervalos se sobrepõem e não foi demonstrado ganho do
protótipo. A revisão posterior identificou riscos e ele foi retirado.
Os logs mencionados ficam fora do Git, em
`${XDG_STATE_HOME:-$HOME/.local/state}/cool_fox/logs/run/`, e podem não estar
disponíveis em outras máquinas.

## Experimento distinto: abertura agrupada

O protótipo removido `VICTORY_GUARD_BATCH_OPEN=1` agrupava aberturas adjacentes
sob condições restritivas. No teste local anterior quase não apareceram
blocos contíguos elegíveis; não há ganho comprovado. Não confundir essa opção
com `VICTORY_GUARD_BATCH_PROTECT`, que atua no fallback da proteção.

## Trabalho pendente

1. Investigar as falhas de testes de buracos e remapeamento também sem os
   protótipos; manter os dados e testes de outras alterações locais.
2. Investigar a tentativa preexistente de proteger a faixa inteira: ela também
   recebe somente a proteção anterior da primeira página, e sua segurança
   depende de sincronização e invariantes que ainda não foram demonstrados.
   A remoção dos protótipos não resolve essa limitação.
3. Antes de propor novo agrupamento, estabelecer sincronização contra mudanças
   de proteção e remapeamentos, testar falhas e restauração, e comparar a
   mesma cena, build, opções, instrumentação e carga externa.
