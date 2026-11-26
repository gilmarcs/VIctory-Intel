# Diagnóstico de FPS no mundo aberto — 26/11/2026

## Contexto e limites

Build Windows x64 `diag` com `diagnostics`, executada pelo Proton Experimental
em Intel Arc B580/Mesa ANV. O usuário confirmou o mundo aberto e relatou não
notar diferenças visuais no teste de buffers. Os arquivos de log ficam fora do
Git em `~/.local/state/cool_fox/logs/run/`; os horários nos registros são UTC.
As cenas, o estado de carregamento e a compilação de pipelines variaram; os
números abaixo são amostras, não benchmarks controlados nem prova visual.

O launcher já usa `VICTORY_TEXTURE_NO_WAIT=1`: evita esperar a estabilização
do remapeamento em certas leituras, mas não elimina consultas write-watch ou
contenção nos guards. A camada de debug D3D12 solicitada não estava disponível
(`0x887A002D`); DRED foi ativado na primeira execução instrumentada.
A integridade completa dos arquivos do jogo não foi confirmada. Abrir um RPF
ou ter arquivos abertos em leitura pelo qBittorrent não confirma sua integridade.

## Execuções

| Log local | Opções sobre o launcher | Observação |
|---|---|---|
| `run-20261126-161421.log` | timing GPU e DRED ligados | Mundo aberto confirmado; quadros de 7–13 s, aproximadamente 0,1 FPS. |
| `run-20261126-162822.log` | `VICTORY_GPU_TIMING=0 VICTORY_D3D12_DRED=0` | Mundo aberto confirmado; amostras de 11,0 e 16,4 s. O problema persiste sem esses dois diagnósticos. |
| `run-20261126-163350.log` | como anterior, mais `VICTORY_GUARD_PROFILE=1` | Perfil de guards no mundo aberto; a instrumentação acrescenta custo. |
| `run-20261126-163902.log` | como referência sem timing/DRED, mais `VICTORY_CARD_BUFFERS_MIN_MB=1` | Amostras de 12,8, 18,2 e 20,8 s; sem ganho demonstrado e sem diferença visual relatada. |

A tentativa anterior `run-20261126-161125.log` terminou com SIGKILL sem causa
confirmada; não houve registro de OOM ou reset de GPU na consulta ao journal
do kernel. A primeira tentativa foi interrompida por limite de tempo configurado
incorretamente. Não confundir nenhum dos dois encerramentos com a causa do FPS.

## Resultados

- Na referência sem timing/DRED, um quadro verificou 9,4 GB de memória, quase
  todos inalterados. A espera acumulada pelo lock dos guards foi 3,7 s somando
  threads; em outra amostra foi ainda maior. Esses tempos podem se sobrepor e
  exceder a duração do quadro.
- No perfil detalhado, um intervalo registrou 1.313 chamadas `open_block` com
  3,19 s decorridos acumulados; o lock ficou 3,02 s no caminho `write` e 1,05 s
  em `protect`. Não somar chamadas e posse de lock: elas se sobrepõem e incluem
  desescalonamento e custo da instrumentação. Em outro intervalo, `protect`
  superou `write`: a proporção varia.
- `watch.rs` invalida estado de aliases antes de liberar escrita e abre cada
  bloco com `VirtualProtect` sob o lock global. Mover chamadas para fora do
  lock ou agrupá-las exige garantias contra remapeamento e mudança de proteção.
  Ver `guards-protecao-experimentos.md`: protótipos anteriores foram removidos,
  sem ganho demonstrado e com riscos de correção.
- Um quadro instrumentado de 8,9 s registrou 4,3 s de preparação e 3,1 s de
  cópia de volta; a GPU ficou ociosa durante grande parte do quadro. Também há
  dispatches compute custosos: o gargalo não é exclusivamente a CPU.
- Com limite de buffer de 1 MiB, mais cópias passaram a ser comparadas na GPU,
  mas centenas de MB ainda foram comparados na CPU e a contenção permaneceu.
  Carga e pipelines variaram; não há prova de ganho nem de regressão causada
  somente pela variável. Não torná-la padrão com base nessas amostras.
- No fim desse teste, a fila de texturas tinha 35.990 entradas: 16.445 prontas,
  18.634 aguardando memória e uma parcialmente carregada. As contagens são
  categorias de estado e prontidão, não equivalem a texturas efetivamente
  visíveis. A atualização somou 6,7 s entre threads, dos quais 6,5 s em
  consultas ao write-watch/guards; os valores podem se sobrepor.
- Desenhos descartados precisam ser separados por motivo: alguns aguardavam
  pipelines, outros eram de uma etapa do construtor de ray tracing desativada.
  Não atribuir todos a falha de compilação.

## Próximos passos

1. Rastrear `TextureQueue::due` e a atualização de cada textura para distinguir
   consultas efetivamente repetidas de consultas necessárias. A fila já
   prioriza reconsultas e alterna entradas novas e antigas; usa número de
   escrita para não descartar atualização concorrente, backoff para memória
   ausente e notificações de remapeamento. Não mudar a ordem às cegas.
2. Identificar escopo e invalidação de qualquer deduplicação: escrita, aliases,
   remapeamento, memória não comprometida e alteração concorrente devem invalidar
   a resposta. Medir chamadas, bytes, tempo e lock antes de propor mudança.
3. Testar a correção com aliases e remapeamentos concorrentes, incluindo os
   testes de buracos já observados como falhos mesmo sem protótipos (causa
   ainda não determinada). Não ignorar nem atribuir falhas sem comparação.
4. Comparar base e mudança na mesma build, cena, tempo de carregamento,
   instrumentação e carga externa; conferir visualmente além de FPS.

## Primeira leitura do código da fila

`descriptor/host.rs:TextureQueue::due` percorre as entradas prontas, ordena
reconsultas e entradas novas separadamente, serve primeiro reconsultas e
intercala a escrita mais nova e a mais antiga das entradas novas. A lista
carrega a geração `write`; `same`, `done` e `later` protegem contra concluir uma
entrada que tenha sido reescrita enquanto o passe trabalhava. Esses detalhes
corrigem a descrição simplificada de que a fila serve apenas as mais recentes.

Em `draw.rs:heap_textures`, entradas completas podem ser reexaminadas por
fingerprint porque o título pode preenchê-las após criar a view sem notificação.
Entradas parciais ou ausentes consultam memória mapeada e têm backoff; eventos
de remapeamento ou escrita as tornam elegíveis novamente. O orçamento comum
usa 250 ms por passe por padrão e pode triplicar com backlog; é verificado
entre entradas, portanto uma única operação lenta pode ultrapassá-lo. Bakes
usam sincronização e orçamento próprios porque consomem os dados imediatamente.

Hipóteses a medir, não correções demonstradas: custo de ordenar toda a fila sob
o lock para retirar apenas parte; chamadas repetidas para views que compartilham
memória; e operações individuais que ultrapassam o orçamento. A contagem de
entradas na fila não mede quantas consultas idênticas são feitas. Antes de
compartilhar resultados, é preciso provar que sua validade abrange escritas por
aliases, mudanças de mapeamento e novas gerações de descritor; eliminar
fingerprints ou reconsultas apenas por desempenho pode deixar texturas antigas.

## Perfil da fila no mundo aberto

Com `VICTORY_TEXTURE_QUEUE_PROFILE=1`, timing GPU e DRED desligados, o usuário
confirmou o mundo aberto. A instrumentação é adicional e altera os tempos:

- Em um passe normal do quadro 1732, 1.304 consultas `mapped_bytes`, 729
  repetindo exatamente endereço e comprimento dentro daquele passe; 802 ms
  acumulados nas consultas e 891 ms de passe com orçamento de 750 ms.
- Em um passe de bake do mesmo quadro, 1.234 consultas, 808 repetidas, 1.463 ms
  nas consultas e 1.565 ms de passe com orçamento de 1.500 ms.
- Noutro passe normal, 609 consultas, apenas 16 repetidas; duas entradas
  levaram 431 e 539 ms, esta última com 538 ms na consulta de mapeamento.
  O orçamento é verificado entre entradas, logo uma entrada pode ultrapassá-lo.
- A seleção sob lock, incluindo ordenação e census, levou aproximadamente
  1,8–4,1 ms nas amostras com cerca de 30 mil entradas; não explica sozinha
  quadros de vários segundos.

`mapped_repeated` mede apenas pares exatos `(endereço, comprimento)` por
invocação de `heap_textures`, não sobreposição parcial, nem repetição em passes
diferentes ou consultas write-watch. Repetição não prova que o resultado seja
reutilizável: mapeamentos podem mudar entre consultas. Os tempos individuais
incluem desescalonamento; chamadas de passes concorrentes podem se sobrepor.
O perfil mede o processamento depois da seleção, não o custo de selecionar.

As duas primeiras tentativas com perfil caíram antes do mundo aberto por
`0xc0000005` em `victory-image-b99f876b74d4a77f.dll+0x2215c35`; uma tentativa
com a mesma build e perfil desligado também caiu no mesmo ponto. Outra execução
posterior sem perfil avançou, então a causa e eventual influência de timing
permanecem desconhecidas. O perfil bem-sucedido não prova estabilidade.

Próxima investigação: estudar `texture::mapped_bytes` e a sincronização de
eventos de mapeamento para desenhar deduplicação com invalidação verificável;
medir primeiro se consultas repetidas retornam o mesmo resultado e sua parcela
de tempo. Não reutilizar a resposta apenas por ter o mesmo endereço e tamanho.

### Sincronização e perfil adicional

`inplace::shown_bytes` lê o registro de views sob `RwLock`; `title_memory_view`
e `views_gone` o alteram sob o lock de escrita. O lock protege cada consulta,
mas é liberado antes da próxima. Em `xmem`, o mapeamento efetivo antecede as
notificações `title_memory_remapped` e `title_memory_view`, que são etapas
distintas. Além disso, quando não há view conhecida, `mapped_bytes` consulta
memória por outros caminhos, inclusive o sistema. Portanto, uma geração apenas
do registro de views não demonstraria validade para todos os resultados nem
fecharia por si só a janela entre operação e notificação. Escritas e aliases
também exigem suas próprias garantias; igualdade de bytes mapeados não prova
igualdade do conteúdo.

O perfil opcional foi ampliado, sem reutilizar resultados: `mapped_repeat_changed`
conta respostas diferentes em relação à consulta anterior do mesmo par dentro
do passe; `mapped_repeat_ms` soma o tempo das consultas repetidas. Igualdade
observada não prova segurança para cache, e os tempos incluem desescalonamento
e custo da instrumentação. Os novos contadores ainda precisam ser medidos no
mundo aberto.

Na execução seguinte, com o perfil ampliado e mundo aberto confirmado pelo
usuário, amostras de passes normais mostraram:

- Quadro 1840: 1.094 consultas, 459 repetidas, nenhuma resposta diferente da
  anterior; 538,630 ms nas repetidas de 674,493 ms no total, passe de 811,9 ms.
- Quadro 1841: 1.408 consultas, 831 repetidas, nenhuma resposta diferente;
  340,066 ms nas repetidas de 731,427 ms no total, passe de 811,3 ms.
- Quadro 1842: 1.044 consultas, 535 repetidas, nenhuma resposta diferente;
  3,010 ms nas repetidas de 160,155 ms no total, passe de 492,6 ms. A consulta
  mais lenta levou 152,027 ms.

O tempo das repetições variou muito e inclui espera e desescalonamento; não é
uma previsão de ganho. Essas amostras não demonstram estabilidade futura do
mapeamento nem segurança de reutilização. Logs próximos registraram 0,1–0,4
FPS, com quadros de 2,4–7,6 s; não são uma comparação controlada de desempenho.

Para a próxima coleta, o mesmo perfil foi ampliado para registrar `views_hit`,
`views_miss`, `views_wait_ms`, `views_search_ms` e `fallback_ms` nas consultas
da fila. O tempo de espera pelo lock inclui desescalonamento; o tempo de busca
inclui instrumentação, e o fallback reúne os caminhos de consulta de memória
sem separá-los. Os tempos não são CPU time e não provam causalidade. Nenhuma
resposta é reutilizada.

Coleta seguinte, mundo aberto confirmado, perfil ligado (passes normais):

- Quadro 3795: 623 hits e 34 misses; 0,154 ms esperando o lock,
  0,797 ms na busca e 640,722 ms nos fallbacks, de 641,767 ms totais.
- Quadro 3797: 1.491 hits e 342 misses; 0,618 ms esperando o lock,
  2,110 ms na busca e 704,048 ms nos fallbacks, de 707,102 ms totais.
- Quadro 3799: 1.127 hits e 44 misses; 0,511 ms esperando o lock,
  1,039 ms na busca e 674,288 ms nos fallbacks, de 675,740 ms totais.

Nessas amostras, o tempo decorrido está quase todo no fallback, apesar de a
maioria das consultas encontrar views. Isso não distingue verificações de
primeira/última página, leitura por blocos ou `VirtualQuery`, nem espera,
desescalonamento e trabalho efetivo. O próximo perfil deve separar os ramos
do fallback e suas chamadas ao sistema antes de escolher uma otimização.

O perfil seguinte acrescenta uma linha `texture mapped fallback profile` por
passe: contagem e tempo da verificação das extremidades (`endpoint`), retornos
sem memória, tipos e tempo de varredura (`block_scans`, `page_scans`,
`query_scans`), blocos examinados e chamadas, falhas e tempo de
`K32QueryWorkingSetEx`, leituras por `host::read_into` e `VirtualQuery`.
Os tempos das chamadas estão contidos nos tempos das respectivas etapas e
não devem ser somados a elas. São tempos decorridos, sujeitos a interrupções
e à sobrecarga do perfil, não tempos de CPU. O perfil cobre somente consultas
da fila instrumentada, não todas as chamadas de `mapped_bytes` do processo.

Na coleta seguinte, com mundo aberto confirmado e perfil ligado:

- Quadro 2257, passe normal: 78 misses nas views, 600,38 ms no fallback;
  257 chamadas de `K32QueryWorkingSetEx`, 600,03 ms nelas; três leituras
  adicionais falharam, em 0,04 ms. Extremos 371,95 ms, varredura 228,37 ms.
- Quadro 2258, passe normal: 357 misses, fallback 715,90 ms; 2.085 chamadas
  de working set em 534,79 ms e 903 leituras em 179,65 ms (900 falharam).
  Extremos 456,95 ms, varredura 258,78 ms.
- Quadro 2259, bake: 205 misses, fallback 650,04 ms; 819 chamadas de working
  set em 640,96 ms e 406 leituras em 8,09 ms (todas falharam). Extremos
  627,15 ms, varredura 22,74 ms.

Nessas amostras cada chamada de working set perguntou por uma única página;
nenhuma chamada `VirtualQuery` foi registrada. Os tempos de chamadas incluem
desescalonamento e contenção dentro do sistema/Proton, não identificados por
este perfil. Extremos e varredura incluem os tempos das chamadas: não somar
as duas formas de detalhamento. Isso aponta para investigar a quantidade e
o custo das consultas ao working set, sem presumir que uma resposta negativa
possa ser reutilizada ou que agrupar consultas preserve a semântica concorrente.

### Pesquisa externa: working set e Wine

- A [documentação Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-queryworkingsetex)
  aceita um array de endereços; o
  [exemplo público da Microsoft](https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/Memory/allocating-memory-from-a-numa-node.md)
  consulta várias páginas numa chamada. O retorno é um retrato momentâneo;
  `Valid=0` não prova que a página esteja descomprometida.
- No [Wine upstream, `get_working_set_ex`](https://github.com/wine-mirror/wine/blob/master/dlls/ntdll/unix/virtual.c),
  o código ordena endereços, entra em `virtual_mutex`, percorre views e, no
  Linux, lê `/proc/self/pagemap` com buffer de 256 entradas. Isso sugere
  contenção e custo por chamada como hipóteses, não como causas comprovadas.
  Não foi confirmada a correspondência com o binário Proton Experimental em
  uso; o branch upstream é mutável.
- Agrupar páginas próximas por textura é candidato experimental; lotes muito
  grandes ou de endereços distantes podem aumentar o tempo sob o lock do Wine.
  Medir tempos de cauda e concorrência, não só número de chamadas.
- Uma versão em lote deve preservar leitura de confirmação para páginas
  `Valid=0`, o caminho de erro da API, os limites e os retornos existentes.
  Testar estado estável, proteção/guarda, mapeamento parcial, aliases e
  remapeamento concorrente. Alterar o momento das consultas não fornece
  atomicidade nem garante igualdade sob concorrência.

### Experimento opt-in de lotes

`VICTORY_MAPPED_BLOCK_BATCH=1` ativa lotes de até 64 páginas representativas
na varredura por blocos de 64 KiB; o padrão continua sendo consultas individuais.
As extremidades mantêm o caminho anterior. Páginas não residentes ainda são
verificadas por leitura segura; se a chamada em lote falhar, o lote é
reconsultado pelo caminho individual. O perfil registra o número efetivo de
chamadas e páginas. A ordem e o instante das observações mudam, portanto o
experimento não oferece equivalência sob remapeamento concorrente.

Há teste de comparação com consultas individuais para uma reserva própria com
blocos comprometidos e lacunas, incluindo faixa desalinhada e mais de 64
blocos. O teste passou no Proton Experimental (1 teste, 0 falhas). Não cobre
aliases nem remapeamentos concorrentes. Falta comparar o jogo com e sem o
experimento.

Primeira coleta com lotes, mundo aberto confirmado e perfil ligado:

- Quadro 1712, passe normal: 1.917 blocos em 46 varreduras, 46 consultas
  de extremidade e 109 chamadas de working set no total. Assim, 63 chamadas
  atenderam os 1.917 blocos; fallback 588,94 ms, dos quais 567,91 ms nas
  extremidades e 20,99 ms na varredura.
- Quadro 1714: 785 blocos em 119 varreduras, 120 consultas de extremidade e
  246 chamadas ao working set no total (126 na varredura); fallback 5,43 ms,
  passe 124,77 ms.
- Quadro 1713: seis extremidades e seis blocos; extremos 348,70 ms, varredura
  0,08 ms. Lotes não eliminam o custo das extremidades.

Os logs registraram aproximadamente 0,1–0,2 FPS e quadros de 5–11,5 s em
amostras próximas. Não demonstram ganho de FPS contra a execução anterior:
o cenário variou e o perfil adiciona overhead. É necessária comparação
controlada com perfil desligado e avaliação visual, além de testes de aliases
e remapeamento concorrente.

O usuário não percebeu alterações visuais na primeira execução com lotes;
isso não comprova correção sob todas as condições. Na referência posterior,
com `VICTORY_TEXTURE_QUEUE_PROFILE=0` e `VICTORY_MAPPED_BLOCK_BATCH=0`,
mundo aberto confirmado, amostras do log `run-20261126-175856.log` mostraram
quadros de 5,6; 8,6; 8,4; 10,9; 10,0 e 7,5 segundos. São amostras esparsas,
não uma distribuição controlada; falta a execução correspondente com lotes e
perfil desligado.
