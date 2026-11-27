# Hang da GPU (DXGI_ERROR_DEVICE_HUNG) no primeiro frame grande do mundo

Investigação de 2026-11-26/27, Windows nativo, Intel Arc B580 (driver 32.0.101.9175),
build `diag` com `diagnostics`. O ponto de partida era "um problema no upload de textura".
O que foi medido está em **Medido**; o que ainda é suposição está em **Hipóteses**.
Logs e dados de teste ficam fora do Git, em `C:\Users\gilma\victory-logs\<nome>\`
(`run.log`, `clean.log` = log sem cores e sem as linhas `victory_gpu::list`).

## Sintoma

- Cerca de 60 a 70 s depois de iniciar (passando a tela inicial com `VICTORY_KEYS`),
  no primeiro frame em que o jogo liga o mundo (1750 a 1900 pipelines usados pela primeira
  vez, frames 1180 a 1400), o device do host é removido: `DXGI_ERROR_DEVICE_HUNG`.
- O mouse e a área de trabalho congelam durante a espera.
- Depois, o título derruba a si mesmo: `victory-image-b99f876b74d4a77f.dll+0x23cf683`,
  `mov dword ptr [0x0], 0x4d8bd421` (escrita deliberada em 0, código fixo). É o
  **watchdog do próprio título**, que dispara quando um frame demora demais. Não é a causa:
  ele também dispara no WARP (software), sem nenhum hang.
- O crash `0xc0000005` aparecia sozinho nas execuções com GBV (a lentidão do GBV basta).

## Medido

| Fato | Evidência |
|---|---|
| O hang é real e a GPU fica **ocupada**, não parada | `typeperf` do motor 3D (`util`): zeros durante a espera e, na primeira amostra depois, 433% (≈ 4,3 s de trabalho acumulado) |
| Não é só lentidão: com `TdrDelay=30` (valendo, depois de reiniciar) a remoção veio só depois de **33,4 s** | `reboot`: "waiting for the card ... for 33387 ms" (antes: 3,5 a 5,3 s) |
| O fluxo de comandos é executável: o WARP termina o mesmo frame (lista de 807 comandos em 440 ms) | `warp` |
| Nenhuma camada de validação acusou nada na Arc; sem GBV não há erro de acesso | `dbgonly`, `launcher` |
| Pular os 23 shaders de ray tracing da submissão **não produz `DEVICE_HUNG`, mas também não é "o hang sumiu"**: o jogo ficou ~40 s parado (último `fps` 02:00:59, crash do watchdog 02:01:39) sem remoção do device, mesmo com `TdrDelay=30`. A GPU não ficou presa, mas o título parou de andar (provável espera por resultado que o shader pulado deixou de produzir). Nenhuma conclusão sobre o RT | `nort` |
| Pular subconjuntos desses shaders **continua travando**: 11, 6, 3 de cada vez; os 3 últimos um a um, juntos e os 6 últimos juntos (`TdrDelay=5` valendo: esperas de 6,3 a 10,3 s) | `bisect_rt`, `three`, `combo` |
| O comando que o DRED/breadcrumbs nomeia como "parado" **muda** conforme o que se remove: upload 16x4 f10, alias copy, `ExecuteIndirect`, `DispatchMesh`. É o comando em execução quando o prazo estoura, não a causa | `skipup2`, `nocopies`, `nomesh2`, `loop1k` |
| A lista que trava é a mesma a cada execução (número 119 a 123 na maioria) e é enorme (380 a 2722 comandos) | todos |

### Descartado (cada item com pelo menos uma execução que ainda travou)

Pool de texturas (`VICTORY_TEXTURE_POOL=0`), espera de residência (`VICTORY_WAIT_RESIDENT=0`),
biblioteca de pipelines em disco (recriada do zero), alinhamento de 4 KiB
(`VICTORY_SMALL_PLACEMENT=0`), aliasing barrier antes do upload (`VICTORY_NO_ALIAS_BARRIER=1`),
idade ou uso do bloco do pool, um upload específico (`VICTORY_SKIP_UPLOAD=16x4:10`),
alias copies (`VICTORY_SKIP_ALIAS_COPY=1`), qualquer um dos 180 shaders da submissão
pulados um a um e todos juntos, `VICTORY_WIDE_GROUPS`, concorrência entre listas
(`VICTORY_GPU_TIMING` já envia e espera lista por lista; sem ele travou igual),
pressão de memória (3,9 GB de 11,3 GB de VRAM), teto de loops traduzidos em 1024
(`VICTORY_SHADER_LOOP_LIMIT=1024`), uma cópia de 16x4 f10 isolada num device D3D12 limpo.

### Sintético (`crates/victory-gpu/examples/synth_upload.rs`)

Cópias de 896 bytes (16x4 R16G16B16A16_FLOAT, offset múltiplo de 512) para texturas
colocadas, na Arc: `placed64`, `placed4`, `churn` (600 mil cópias cada) e `persist`
(300 listas x 6 rodadas x 600 cópias com leitura por compute) passam sem hang. O padrão
de cópia, sozinho, não trava o driver. O cenário `committed` estoura o tempo por criar
recursos demais e não vale.

## O ray tracing não está desativado (contexto, **sem prova de que cause o hang**)

Pelo `docs/fork-ray-tracing.md`, o jogo não usa DXR: o traversal é compute comum
(pilha em memória compartilhada, endereços brutos do título) e 52 programas "builder"
montam as estruturas a cada frame. Por padrão **só o builder** fica desligado
(`VICTORY_BVH_BUILDER=1` o liga; um builder parcial já escreveu limites inválidos e derrubou o
título). Os programas de **trace** continuam sendo traduzidos e despachados, sobre
estruturas dinâmicas que nunca foram construídas.

Os 23 shaders descartados no teste `nort` (`scratchpad\rt_hashes.txt`): 14 `sr_tracingCS_*`
(ClearRayBins, ClearRayCounts, DeferredFallbacksAlpha_SkyProbe, GenerateRayBinOffsets,
GenerateRayBins, RayBudgeting_Accumulate/Cull_RayBinning/Fill, RayMiss, RayPrePass_Alpha,
RaytraceVBuffer_RayBinning_ShaderBinning_Mixed_WithFar, ResolveScreenSpace, Tiling_Clear,
Tiling_Generate_Alpha), 6 `shader_binning`, `sr_denoising`, `scattering_transmittance_systemCS_SortRayTiles`
e `..._ClassifyRayTiles`.

Busca binária entre os 23 (`scratchpad\bisect_rt.sh`, `victory-logs\bisect_rt\result.txt`): as
rodadas 1 a 3 deram HANG (a rodada 4 saiu inválida, 31 binds). Os 3 candidatos que sobraram
(`dd411d22d78b3625` GenerateRayBins, `f41afb810c1e29c6` GenerateShaderBins_Tiled,
`f9ed53343e371f47` ClearRayBins) deram HANG pulados um a um, juntos e junto com os outros 3 do
grupo de 6 (`victory-logs\three`, `combo`). **A busca falhou**: ou a hipótese de um único
culpado é falsa, ou o RT não é o gatilho.

**Correção:** uma versão anterior deste documento dizia que pular os 23 shaders "fazia o hang
sumir". Isso estava errado: nessa execução o título parou por ~40 s e caiu pelo watchdog antes
de qualquer remoção do device (ver a tabela acima). O critério "sem `DEVICE_HUNG` = OK" dos
scripts de busca só vale se o log mostrar que o jogo continuou rodando (fps), o que eles não
conferem.

## Builder ligado (`VICTORY_BVH_BUILDER=1`, `VICTORY_BVH_TRACE=1`)

Duas execuções (`victory-logs\bvh`, `bvh2`; `TdrDelay=5` valendo): **nenhum `DEVICE_HUNG`, nenhum
device removido, o computador responde**, mas a janela do jogo fica **preta** e o título cai pelo
watchdog (`+0x23cf683`) cerca de 30 s depois do último frame.

- `bvh`: fps normal (30) até 02:22:27, nenhum frame depois; crash no meio da criação de pipelines.
- `bvh2` (segunda execução): fps caiu para 10,9 em 02:25:27 e parou; crash em 02:25:59.
- Nos dois casos o log mostra shaders de compute sendo preparados e pipelines sendo criadas
  no intervalo sem apresentação. Os totais de 1765 e 1128 linhas `shader translated` incluem
  DXIL lido do cache. Os dois testes terminaram antes de permitir uma comparação do hang
  no primeiro frame grande do mundo.

### O que o log mede no cache

- **Medido nos logs antigos:** antes da parada, a última amostra de `shaders from the cache and
  translated so far` registrou 1671 acertos e zero traduções novas em `bvh`, 292 acertos e
  32 novas em `bvh2`, e 1677 acertos e zero novas em `reboot`. A contagem de
  `shader translated` era de shaders preparados pela primeira vez no processo, inclusive
  os encontrados no cache. Ainda há trabalho de análise de recursos e criação de pipeline
  quando o DXIL vem do disco. A amostragem por frame deixa de avançar se a apresentação para.
- **Defeito corrigido em `apps/victory/src/shader.rs`:** para um DXBC não compactado, o
  scanner alimentava a chave com todos os bytes do cabeçalho até o fim do arquivo. Agora
  recorta o contêiner pelo tamanho validado no cabeçalho DXBC. O teste
  `raw_shader_uses_only_the_container_bytes` cobre o caso com bytes após o contêiner.
  Os registros SGD5 compactados já fornecem os bytes descompactados de cada registro.
- A chave é composta pelos bytes do contêiner, a versão do tradutor, variáveis de tradução
  (como `VICTORY_WAVE_LDS_WIDE`) e as cinco configurações do host. O builder em si
  (`VICTORY_BVH_BUILDER`) fica fora da chave. Para contar os acessos mesmo quando a
  apresentação para, habilite `VICTORY_LOG=info,victory_gpu::shader_cache=debug` e conte
  `shader from the cache` e `shader translated and cached`. Traduções recusadas voltam a
  ser tentadas em outro processo.
- `victory warm` recria as pipelines que constam no catálogo gravado com
  `VICTORY_PIPELINE_BOOK=1`. Isso aquece o cache do driver. A biblioteca D3D12
  serializada pelo Victory tem contadores e persistência próprios.
- **Medido na Arc em `cache-arc/shaders-fixed-1.log` e `shaders-fixed-2.log`:**
  `shaders shaders.rpf game_scarlett_master.exe XboxBvhBuilder.dll --jobs 10`, com
  `VICTORY_WAVE_LDS_WIDE=1`, ondas de 16 a 32 lanes e limite de 1.000.000 de views.
  A primeira passagem corrigida teve 23.031 acertos e 2.716 traduções novas em 53 s.
  A passagem seguinte teve 25.747 acertos e zero traduções novas em 4 s. Em ambas
  as passagens, 209 contêineres foram recusados pelo tradutor.
- **Defeito corrigido em `apps/victory/src/warm.rs`:** em `warm-initial.log` o
  catálogo vazio foi detectado, mas a abertura de `warm-done.txt` falhou com erro 3
  porque o diretório `pipelines` ainda não existia. O comando agora cria o
  subdiretório pelo mesmo mecanismo de configuração dos demais caches.
  `warm-fixed.log` confirmou a execução em cache sem catálogo, com zero pipelines.

### Arc com DXIL pré-traduzido

- **Medido em `cache-arc/book-on`**, com o builder solicitado e o catálogo sendo
  registrado. O jogo chegou a 1800 linhas `first bind`, com o pico no frame 1844.
  Houve 982 leituras do cache e 20 traduções novas. A compilação das pipelines ainda
  ocupou parte do intervalo após o último FPS (9,6 às 23:35:42, hora local).
  Às 23:36:11 o log confirmou que as 53 pipelines do builder estavam prontas e
  que o builder rodou. Às 23:36:24 o device foi removido com `DXGI_ERROR_DEVICE_HUNG`,
  depois de 8586 ms esperando uma submissão de 29 listas. O contador 3D do PID
  marcou 0 durante as últimas amostras da espera e 728,54% na amostra da remoção,
  compatível com tempo de GPU acumulado durante o hang. O jogo terminou com
  `0x887A0005` 3,6 s depois. Não houve nova amostra de FPS após o frame grande.
- A compilação no primeiro uso continuou mesmo com o DXIL em disco. No código,
  `builder_runs` chama `make::now` para as pipelines do builder, o que pode
  compilar na thread atual ou esperar uma compilação já iniciada. O catálogo
  reuniu 83.575 entradas, das quais 1277 estavam na lista de início antecipado.
  A comparação abaixo usa `warm --used` para aquecer as da abertura.
- **Medido em `cache-arc/warm-used.log`** com `warm --jobs 2 --used`.
  O aquecimento sem filtro foi interrompido depois de registrar 296 pipelines
  processadas. Com o filtro, foram processadas as 981 restantes da lista de
  início antecipado em 147 s, com uma recusa e nenhum processo interrompido.
- **Medido em `cache-arc/builder-off`**, depois do aquecimento. Foram 1821 linhas
  `first bind` (1751 no frame 1863), 1315 leituras do cache, zero traduções novas
  e uma recusa. O último FPS foi 31,1 às 23:40:55. O device foi removido às
  23:41:08, depois de 6606 ms de espera pelo host da GPU em 29 listas, sem
  chamada ativa na biblioteca. O contador 3D do PID voltou de zero com uma
  amostra de 573,39% perto da remoção, novamente compatível com tempo acumulado.
- **Medido em `cache-arc/builder-on`**, no mesmo cache compartilhado, após
  `builder-off`. Foram 1805 linhas `first bind` (1745 no frame 1891), 1503 leituras
  do cache, zero traduções novas e uma recusa. Às 23:42:31 o builder rodou com as
  53 pipelines prontas. Uma submissão de uma lista com trabalho do builder
  terminou em 2681 ms. A thread de submissão esperou 25.424 ms pela GPU em outra
  submissão, de 29 listas, sem chamada ativa na biblioteca. O device foi removido
  às 23:43:01 com `DXGI_ERROR_DEVICE_HUNG`. O contador 3D do PID ficou em zero na
  espera e marcou 2192,83% na amostra da remoção. O último FPS foi 4,2 às 23:42:25.
  Os tempos de espera acima vêm do host e não medem o limiar efetivo do TDR.
- **Comparação dos controles:** a submissão lenta com builder ligado incluía
  todas as 158 combinações de shaders de pipeline nomeadas na submissão sem o
  builder (287 no total com ele). Em ambos, o DRED parou no comando 333 de 433
  da lista principal (IDs 122 e 103), com marca 91. A próxima nota é um draw
  indireto com `ed734c953c0eac78` / `75c0dbd88cca7374`. Em `book-on` o ponto
  registrado foi outro. Essa coincidência dá uma combinação a testar, sem
  identificar o comando que causou o hang.

### Combinações com o builder ativo

- **Medido em `cache-arc/pair-trace`:** omitir ao mesmo tempo
  `a1bf269d5b19f025` (SortRayTiles) e `6d7fe7745ab9c6d8` (RaytraceVBuffer)
  produziu 1792 linhas `first bind` (1735 no frame 1826), 1504 leituras do
  cache e zero traduções novas. Os dois hashes ficaram fora do conjunto de
  pipelines da submissão que travou. Com as 53 pipelines do builder prontas,
  houve remoção às 23:44:35, após 7543 ms de espera em 29 listas. O contador
  3D marcou 651,59% na primeira amostra após a remoção.
- **Correção no procedimento:** as primeiras execuções usaram diretórios de
  dados vazios por teste. Isso fez o título repetir a configuração de tela e
  o aceite da EULA, introduzindo interação humana e variação no tempo de
  abertura. A primeira tentativa do par misto (`pair-mixed`) foi interrompida
  com 33 binds. A repetição usa `VICTORY_DATA_DIR` apontando para
  `cache-arc/configured-data`, com uma cópia dos diretórios `saves` e
  `storage` de `builder-on`. Os logs continuam separados por execução.
- **Medido em `cache-arc/pair-mixed-configured`:** neste teste foram omitidos
  `a1bf269d5b19f025` (compute) e `1fc275e1f758fde5` (pixel observado em
  `book-on`). Com os dados já configurados, o jogo produziu 1879 linhas
  `first bind` (1838 no frame 1262), 1593 leituras do cache e zero traduções
  novas. Os dois hashes ficaram fora da submissão que travou. Com o builder
  ativo, a remoção veio às 23:46:30 após 8583 ms em uma submissão de 29 listas.
  O contador 3D marcou 743,39% na primeira amostra após a remoção.

O hang se reproduz com DXIL em cache e com o builder ativo ou desligado.
Os dois pares testados também deixaram o hang ocorrer. A thread de submissão
estava esperando a GPU nos dois controles aquecidos. A causa da falha da
submissão e a dependência específica que bloqueia o título continuam abertas.
Os testes terminaram sem uma apresentação confirmada depois do primeiro
frame grande, portanto a imagem e os limites das estruturas BVH na Arc
continuam sem avaliação. O builder permanece opt-in.

### Repetição com o mesmo perfil e argumentos indiretos

- **Controle `cache-arc/configured-control`:** perfil reaproveitado de
  `configured-data`, builder ativo com 53 pipelines, `VICTORY_INDIRECT_BATCH=1`.
  O jogo registrou 1779 linhas `first bind` (1733 no frame 1244), 1503
  leituras do cache e zero traduções novas. Houve `DXGI_ERROR_DEVICE_HUNG`
  após 8233 ms de espera por 29 listas, sem chamada ativa na biblioteca.
- O DRED do controle marcou o comando 431 de 531, operação 8
  (`CopyTextureRegion`), num upload 16×4 f10. A marca 123 confirma a
  conclusão das operações anteriores à nota 122, que é o upload. As notas
  123 e 124 são draws indiretos com `ed734c953c0eac78` /
  `75c0dbd88cca7374`. O código grava a marca antes da operação que ela
  anota. O intervalo seguinte inclui o upload e pode incluir comandos
  auxiliares antes do próximo draw. O DRED e as marcas delimitam essa
  região da lista.
- **`cache-arc/configured-pair-75c0`:** com `SortRayTiles`
  (`a1bf269d5b19f025`) e o pixel shader `75c0dbd88cca7374` omitidos juntos,
  o jogo registrou 1789 binds (1741 no frame 1243), 1509 leituras do cache
  e zero traduções novas. O builder tinha 53 pipelines prontas. A GPU
  travou após 7794 ms de espera por 29 listas. Os dois hashes estavam
  ausentes da submissão. O DRED voltou a marcar um upload 16×4 f10 e os
  draws indiretos seguintes usavam outro pixel shader, `1fc275e1f758fde5`.
  O contador 3D marcou 678,34% na primeira amostra após a remoção. O usuário
  observou a janela branca desde a abertura e congelamento do mouse nessa
  execução. Os FPS registrados antes do hang medem a apresentação, que pode
  ocorrer mesmo com a janela branca.
- **`cache-arc/configured-no-batch`:** com `VICTORY_INDIRECT_BATCH=0`, todos os
  shaders habilitados e o builder ativo, o jogo registrou 1861 binds (1809 no
  frame 1277), 1591 leituras do cache e zero traduções novas. Houve
  `DXGI_ERROR_DEVICE_HUNG` após 6693 ms de espera por 21 listas. O contador 3D
  marcou 568,13% na primeira amostra após a remoção.
- Sem agrupamento, o DRED marcou o comando 989 de 992, operação 7
  (`CopyBufferRegion`), numa lista com draws indiretos de mesh da pipeline
  `45f081396c2c59ba` / `59b1ba494f18c894`. O comando 986 era o último
  `ExecuteIndirect` anotado. As 83 combinações de shaders nomeadas na
  submissão de 21 listas não apareceram entre as 281 do controle de 29 listas
  nem entre as 285 do teste com o par omitido. O teste mostra que desligar
  o agrupamento também termina em hang, em outra submissão do primeiro frame
  grande. A relação entre os dois hangs segue em aberto.
- **Hipótese para a operação 989:** o fim da lista tem duas barreiras antes
  de `CopyBufferRegion` e outra depois. Em `HostList::close`, esse padrão é
  compatível com a transição do scratch de argumentos e o readback de 16
  bytes das estatísticas de orçamento em `card_budget_out`. O breadcrumb
  não registra os buffers envolvidos. `VICTORY_INDIRECT_CARD_BUDGET=0`
  mantém o orçamento em memória mapeada pela CPU e remove a inicialização
  na GPU e essa cópia. O teste varia o caminho completo do orçamento.
- **`cache-arc/configured-no-batch-mapped-budget`:** sem agrupamento e com
  o orçamento mapeado, o jogo registrou 1851 binds (1805 no frame 1198),
  1583 leituras do cache e zero traduções novas. Com o builder ativo,
  houve remoção após 6506 ms de espera por 21 listas. O contador 3D
  marcou 565,94% na primeira amostra após a remoção. A submissão incluía
  todas as 83 combinações de shaders da variante sem agrupamento com
  orçamento na GPU, mais uma. O readback do orçamento pode ser removido
  e a falha ainda acontece nessa submissão.
- Com o orçamento mapeado, o DRED marcou `ExecuteIndirect` no
  comando 707 de 986 da lista 30. O draw aponta para
  `4e35b816f2f91d68` / `659fe9cae59dbd88`, identificados offline como
  `VS_OctaVsplitWind_e6901d4b_Wrapped` e `PS_OctaVsplit_60c60b2c_Wrapped`.
  Os dois breadcrumbs da lista 30 correspondem a comandos diferentes
  que renderizam a mesma família de alvos 1280×720 (formatos 27, 28, 34).
  O log de nomes está em `cache-arc/mapped-budget-shader-names.txt`.
- **Nomes confirmados offline em `cache-arc/dred-shader-names.txt`:** o vértice
  comum aos draws perto do upload é `ptfx_VS_aba1527f_Wrapped`. Os pixels
  `75c0dbd88cca7374` e `1fc275e1f758fde5` são
  `ptfx_PS_AlphaPrepass_aba1527f_Wrapped` e `ptfx_PS_aba1527f_Wrapped` da
  mesma família. A pipeline de mesh sem agrupamento usa
  `MS_VS_OctaSLOD_5fd19871_Wrapped` e `PS_OctaSLODReZ_f191416b_Wrapped`.
  O rastreamento do produtor dos argumentos ainda está em aberto.

### Controles adicionais do orçamento e dos draws Octa

- **`cache-arc/configured-batch-mapped-budget`, 2026-11-27:** no perfil
  reaproveitado, `VICTORY_INDIRECT_BATCH=1` e
  `VICTORY_INDIRECT_CARD_BUDGET=0`. Foram 1883 binds (1842 no frame 1260),
  1596 leituras do cache e zero traduções novas. Com 53 pipelines do
  builder prontas, houve `DXGI_ERROR_DEVICE_HUNG` às 00:01:25 após 7584 ms
  de espera pela submissão de 29 listas. O contador 3D marcou 659,80%
  na primeira amostra após a remoção.
- A submissão tinha 283 combinações de shaders, incluindo todas as 281
  do controle agrupado com o orçamento na GPU. O DRED marcou novamente
  um upload 16×4 f10 (`CopyTextureRegion`, comando 418 da lista 106),
  seguido de draws `ptfx` nas notas. Mover o orçamento para memória
  mapeada preservou a falha também com o agrupamento padrão.
- **`cache-arc/configured-no-batch-mapped-skip-octa`:** sem agrupamento,
  com orçamento mapeado e omitindo `45f081396c2c59ba` e `4e35b816f2f91d68`.
  Foram 1783 binds (1735 no frame 1224), 1518 leituras do cache e zero
  traduções novas. A submissão de 23 listas que substituiu a de 21 terminou
  em 6622 ms. Ela incluiu 74 das 84 combinações da submissão que travava no
  controle sem agrupamento. Além das duas pipelines Octa selecionadas,
  outras oito combinações ficaram fora dessa submissão.
- Esse teste avançou até uma submissão de 29 listas com 280 combinações,
  das quais 274 também estavam no controle agrupado. A remoção veio às
  00:02:48 após 10.895 ms de espera. O contador 3D marcou 933,24% na
  primeira amostra após a remoção. O DRED voltou ao upload 16×4 f10
  (`CopyTextureRegion`, comando 428 da lista 103), seguido dos draws
  `ptfx` nas notas. A progressão entre submissões mostra um efeito da
  intervenção. A diferença de oito combinações limita a atribuição desse
  efeito às duas pipelines selecionadas.
- **`cache-arc/configured-skip-octa-ptfx-upload`:** nas mesmas condições,
  também foram omitidas as pipelines dos pixels `75c0dbd88cca7374` e
  `1fc275e1f758fde5`, e o seletor `16x4:10` registrou pelo menos 300
  uploads descartados. O jogo atingiu 1805 binds (1759 no frame 1250),
  com 1530 leituras do cache e zero traduções novas. Com o builder
  ativo, houve remoção às 00:04:22, após 8571 ms de espera por 7 listas.
  O contador 3D marcou 740,14% na primeira amostra após a remoção. Os
  quatro hashes omitidos estavam fora dessa submissão. O DRED marcou
  outro upload, de 512×513 f87, no comando 78 da lista 88.
- A submissão de 7 listas tinha 27 combinações de shaders, sem
  interseção com as das submissões de 29 listas. A identificação offline
  em `cache-arc/skip-octa-ptfx-upload-shader-names.txt` encontrou draws
  `CascadeShadows` e os três computes `BatchCullInitArgs`,
  `BatchCullMeshlets_ParaboloidShadows` e
  `BatchCullPreSkinnedMeshlets_ParaboloidShadows`. Na lista 88, o
  `BatchCullInitArgs` (`9146291b38ea9ce1`) foi anotado no comando 41,
  antes dos draws indiretos de mesh dos comandos 47 a 71. Ele também é
  o único hash de shader compartilhado com a submissão de 21 listas
  do controle sem agrupamento com orçamento mapeado. A ordem e os
  nomes dos shaders dão um produtor candidato. O log existente não
  identifica o endereço UAV completo para ligá-lo aos registros
  consumidos pelos draws.

## Hipóteses (não provadas)

1. Um shader de trace percorre estruturas inexistentes (zeradas ou antigas) em loops que
   não terminam na Arc. O teto de 1024 não o encerrou, o que pode ser porque o traversal tem
   laços aninhados (1024 por laço ainda é muito trabalho) ou porque o teto não age sobre o
   padrão de saída desses laços. Não verificado.
2. Acesso a memória inválido (falha de página que a Arc fica atendendo) que o WARP
   mascara devolvendo zeros. O DRED não reporta falha de página nesta GPU.
3. Bug do driver da Arc com um padrão de código desses shaders.

### Varredura estática dos 180 shaders da submissão (3 agentes Haiku, só leitura)

Desassembly via `shader-info --disasm` (GFX10 do console, não a tradução). **Nenhuma
espera ativa** (`s_sleep`, polling com glc, barreira em loop) nos três grupos. Candidatos
"dependentes de dados", todos marcados como incertos pelos agentes:
`sr_tracingCS_RaytraceVBuffer_RayBinning_ShaderBinning_Mixed_WithFar` (`6d7fe7745ab9c6d8`,
sete loops com `ds_wrxchg_rtn_b32` e sentinela 3), `scattering_transmittance_systemCS_SortRayTiles`
(`a1bf269d5b19f025`), `Gather_Classified_*` (13 shaders com contador float e limite vindo de textura:
NaN ou acima de 2^24 nunca sai), `DepthOfFieldCS_TileClassificationNeighborhoodBoundary`
(`f8a6133bfedc5759`), `PS_AlphaPrepass_26a320a2` (`783771bb40fcc6c9`). Nada disso foi rastreado
até a comparação de saída nem confirmado por execução.

## Outros achados

- `vfx_triangleClip:CS_DefragHeap_ComputeRelocations` (`02384a60fda6f2d0`) é recusado no wave32
  (cross-half + LDS, grupo de 128). `widen` só aceitava grupos de 64; com `VICTORY_WIDE_GROUPS=1`
  (commit `107ad07`) traduz, valida no DXC e cria a pipeline. **Não é a causa do hang** (o hang
  é idêntico com a flag em 0 e 1). Os resultados numéricos do shader não foram comparados e o risco
  de deadlock de barreira em branch wave-wide não foi descartado; por isso a flag é opt-in.
  Os scripts `.ps1` locais a ligam por padrão (a decidir).
- Biblioteca de pipelines em disco: 78 de 56642 páginas liam como zero no arquivo mapeado.
  Recriar a biblioteca não mudou nada.
- Sem input o jogo fica na tela inicial (7 binds). Use `VICTORY_KEYS='15-180/2:Enter'`
  para qualquer teste que precise chegar ao mundo.
- `texpool.rs` não é referenciado por nenhum módulo (código não compilado).

## Ferramentas de diagnóstico deixadas no código (commit `09bc2e8`, mais `texture.rs` local)

Todas atrás da feature `diagnostics` ou de variável de ambiente:

| Variável / item | Efeito |
|---|---|
| relatório de device removed | lista os 40 comandos anteriores ao parado, por nome (`before_stop`) |
| `VICTORY_MARK_DONE=1` | antes de cada comando anotado, escreve num buffer de readback depois que os anteriores terminam (`MARKER_OUT`); o relatório lê o slot da lista |
| notas de upload | quantas vezes a textura foi enviada, se é a primeira da lista, onde está no bloco (idade, nº cortado), layout, flags, alinhamento |
| `VICTORY_SKIP_UPLOAD=<w>x<h>:<fmt>` | descarta uploads dessas texturas e loga quantos |
| `VICTORY_SKIP_ALIAS_COPY=1` | descarta alias copies (**local, sem commit**) |
| `VICTORY_SMALL_PLACEMENT=0` | texturas do pool a 64 KiB |
| `VICTORY_NO_ALIAS_BARRIER=1` | sem aliasing barrier antes do primeiro upload |
| `VICTORY_SKIP_PIPELINES=<hash,...>` | já existia: descarta os dispatches desses shaders |
| exemplo `synth_upload` | `cargo run -p victory-gpu --example synth_upload -- <placed64\|placed4\|churn\|persist> [n]` |

Observações para quem for usar: com `VICTORY_GPU_TIMING=1` o host envia e espera cada lista
sozinha; as marcas serializam a GPU e podem mudar o ponto do hang; descartar um shader por hash
só vale se o log mostrar que o jogo chegou ao mundo (≥ 1700 `first bind`).

## Mudanças no sistema (a reverter)

`HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers`, feitas com UAC aprovado pelo usuário:
`TdrDelay` e `TdrDdiDelay` (DWORD, segundos). Antes **nenhum dos dois estava definido** (padrões do
Windows: 2 s e 5 s). Valores gravados: 30/30, depois 5/5 (esperado: valem depois de reiniciar).
Para voltar ao padrão, apague os dois valores num PowerShell de administrador:

```powershell
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' -Name TdrDelay,TdrDdiDelay
```

## Como reproduzir e próximos passos

Configuração usada: `VICTORY_D3D12_DEBUG=1 VICTORY_D3D12_DRED=1 VICTORY_GPU_TIMING=1
VICTORY_TEXTURE_NO_WAIT=1 VICTORY_PIPELINE_WORKERS=10 VICTORY_WINDOW_MAX=1920x1080
VICTORY_WAVE_LDS_WIDE=1 VICTORY_KEYS='15-600/2:Enter' VICTORY_MARK_DONE=1`, jogo em
`./target/diag/victory.exe run <exe> --execute --console series-s`, timeout de 250 s
na rodada `cache-arc`. O cache compartilhado foi explicitado em `VICTORY_CACHE_DIR`
e o segundo teste do par misto reaproveitou os dados locais já configurados.

Use a mesma configuração de tradução e a mesma Arc em `shaders`, `warm` e
`run`. A lista de pipelines vem de uma execução de catálogo com
`VICTORY_PIPELINE_BOOK=1`. O comando `warm --jobs 2 --used` usa as
entradas da abertura gravadas nessa lista.

1. Entender o que o título espera quando os 23 shaders de RT são pulados (parada de ~40 s sem
   GPU presa): isso mostra o que esses programas produzem e quem os consome.
2. Rastrear dependências de buffers e recursos entre as pipelines da submissão comum aos
   controles aquecidos. O próximo conjunto de pares pode incluir a pipeline que usa
   `75c0dbd88cca7374` e os produtores dos argumentos indiretos que ela consome.
3. Tratar à parte o watchdog do título (frames de 250 ms a 1 s ou paradas longas o derrubam).

### Rodada de rastreamento de argumentos mesh (2026-11-27)

Na rodada `configured-trace-indirect-uav`, o usuário observou a janela branca durante toda
a execução. O processo foi encerrado com `DXGI_ERROR_DEVICE_HUNG`; contou 1.621 `first bind`,
abaixo do limiar de 1.700 usado aqui para dizer que chegou ao frame grande. Não é um resultado
visual válido nem prova que a tela branca seja causada por este conjunto de shaders. A cópia
configurada de `saves`/`storage` foi reutilizada.

Com batch indireto e orçamento GPU desligados, e os mesmos quatro shaders/upload pulados da
rodada anterior, a submissão de 23 listas levou 6,712 ms e caiu. O DRED marcou a lista 38,
com 7/241 comandos: o comando marcado era um upload de textura `512x513 f87`; antes dele havia
dispatch, duas barreiras e escrita imediata de buffer. O marcador confirma que os comandos
anteriores ao ponto anotado terminaram. Portanto esse registro não identifica o dispatch como
causa; o upload pode apenas ser o primeiro marcador após outro trabalho pendente.

A telemetria opcional de argumentos mesh registrou o endereço `0x948a01e40` dentro do shadow
`0x948a01c20+0x8000` (offset `0x220`, stride 12), consumido por vários pipelines mesh ao longo
da execução. Isso é uma faixa compartilhada de argumentos, não evidência de que o
`BatchCullInitArgs` (`9146291b38ea9ce1`) a escreveu. A instrumentação de raiz UAV não capturou
essas escritas: o shader usa UAV em tabela de descritores. A telemetria de argumentos também
gerou milhares de linhas, por isso foi removida. Ficou apenas um registro opcional dos bindings
completos de compute, filtrado por `VICTORY_TRACE_UAV_SHADER=<prefixo-hash>`, para obter os
descritores UAV em uma futura rodada, caso o usuário queira prosseguir com uma execução do jogo.

### Análise offline de `BatchCullInitArgs`

O container foi extraído sem iniciar o jogo para `cache-arc/batch-cull-init` e decodificado com
`shader-info --disasm`. É um compute nativo GFX10 de 256 bytes, grupo `[64,1,1]`, com um UAV
na tabela (`s0+0`, recurso `u8`) e uma constante raiz (`s1`, `u500`). Não há loop: cada lane
calcula `index = group_id.x * 64 + lane_id` e grava quatro dwords zero em `UAV + index * C`,
onde `C` é a constante raiz. No relatório de execução, `C = 0x20` e o dispatch observado foi
`16x1x1`. Assim, o shader inicializa 1.024 registros, escrevendo 16 bytes a cada 32 bytes,
com footprint de `16 * 64 * 32 = 0x8000` bytes; os últimos 16 bytes do footprint ficam sem
escrita.

Isso coincide exatamente com o tamanho do shadow de argumentos mesh visto na telemetria:
`0x948a01c20+0x8000`, que contém os argumentos consumidos a partir de `0x948a01e40`. A sequência
no DRED também é coerente: `BatchCullInitArgs`, depois `BatchCullMeshlets_ParaboloidShadows`,
depois os draws mesh indiretos. Esses draws usam registros em passos de `0x20`, iguais à constante
de stride de inicialização `C`. A hipótese de que o compute zera os argumentos desse mesmo bloco
passa a ser **muito provável**: combina nome, ordem, footprint e espaçamento. Ainda falta comparar
o endereço inicial do UAV com `0x948a01c20` para confirmação direta. O registro opcional
`VICTORY_TRACE_UAV_SHADER=9146291b38ea9ce1` foi ajustado para imprimir os bindings completos de
compute numa captura futura. A última rodada do jogo mostrou tela branca o tempo todo, então
esta conclusão veio apenas do disassembly e dos registros já existentes.

Os outros dois computes do mesmo conjunto de culling também foram extraídos e decodificados offline.
`BatchCullMeshlets_ParaboloidShadows` declara um UAV na mesma posição de recurso `u8` e outro
UAV `u9`; seu código contém `buffer_store_format_xyzw` para `u8` e `buffer_atomic_add` para `u9`.
`BatchCullPreSkinnedMeshlets_ParaboloidShadows` repete esse par de UAVs e as mesmas instruções
de store e atomic. Isso dá uma função coerente ao bloco: `BatchCullInitArgs` zera os argumentos
em `u8`, e os shaders de culling alocam/gravem argumentos nessa tabela antes dos draws mesh.
O registro focado posterior confirmou o endereço: o UAV de `BatchCullInitArgs` foi ligado a
`0x948a01c20+0x8000`, exatamente o shadow que contém os argumentos mesh em passos de `0x20`.
Isso confirma que o compute inicializa o mesmo bloco consumido pelos draws. O teste com essa
telemetria ainda terminou em `DXGI_ERROR_DEVICE_HUNG`, agora após 1.871 `first bind`; o DRED
parou na lista 209, comando 494/623, um `ExecuteIndirect` de draw indexado. Logo, identificar
essa dependência de dados não prova que `BatchCullInitArgs` seja o causador do hang. Nenhum
arquivo do jogo foi alterado.

### Isolamento do comando de cópia (2026-11-27)

As capturas diferenciais anteriores ainda terminam em `DXGI_ERROR_DEVICE_HUNG` ao remover
`BatchCullInitArgs`, ao pular o upload `512x513 f87`, ou ao pular todos os uploads comuns.
Pular também os alias copies moveu a falha para um upload comum; pular uploads e alias copies
juntos evitou a remoção do device, mas deixou o título avançar até um acesso inválido de CPU
(`0xc0000005`) depois de cerca de três minutos. Isso não é uma execução visual válida, mas
mostra que o hang não depende de um único upload nem exclusivamente do caminho de alias copy.

O exemplo `synth_upload` foi ampliado para aceitar `SYNTH_TEXTURE=WxH:FORMAT` e
`SYNTH_COPIES_PER_LIST`. Na Arc B580, passaram 10 listas de 8 cópias `512x513 f87` nos cenários
`placed64` e `placed4` (o tamanho grande exige alinhamento efetivo de 64 KiB nos dois), 20 listas
de 80 cópias `512x513 f87` e 20 listas de 80 cópias `512x512 f71`, ambas em `placed64`. Também
passaram 10 listas de 400 cópias `512x513 f87` em `placed64` (4.000 cópias) e o cenário
committed para `512x513 f87`. Portanto formato, dimensões, origem em upload buffer, cópia para
textura colocada, volume alto e transição simples de estado não reproduzem sozinhos o hang.

O recurso nomeado pelo DRED no caso `512x513 f87` estava colocado em um bloco do pool de
texturas (`block 1`, peça `0x80000+0x110000`, terceira textura cortada desse bloco) e já tinha
recebido quatro uploads. O DRED aponta a cópia como comando ativo depois de seis comandos
anteriores concluídos, mas os testes sintéticos mostram que isso não basta para atribuir a causa
à instrução de cópia. A diferença ainda não reproduzida é o contexto do jogo: conteúdo e origem
do upload, alocação/reutilização da peça do pool e trabalho GPU em volta (incluindo o dispatch
imediatamente anterior na mesma lista). Próxima captura deve correlacionar a identidade e o
histórico da peça/recurso, staging e barriers com os comandos anteriores; evitar desligar todos
os uploads em uma execução longa, pois essa condição leva a crash de CPU sem imagem utilizável.

### Capturas após o isolamento de cópias (2026-11-27)

Na execução `configured-skip-msr-alpha-prepass`, o filtro foi configurado para
`c7ede3ef60c9fa72` (MSR AlphaPrepass), mas o log não registrou uma queda atribuída a esse
shader. O teste, portanto, não confirma que ele tenha sido omitido. O título chegou ao frame
1241 (1.241 `first bind`) e continuou reportando FPS até a remoção do device. O DRED voltou a
marcar um upload `16x4 f10`, agora na lista 120, comando 431/531. Nos 40 comandos anteriores há
uma sequência de draws mesh, incluindo vários `e135eaf39576263a` e `aa017fca6b1feb21`, além de
draws indiretos; o marcador de conclusão cobre os comandos antes da nota 122. Isso reforça que o
upload é um marcador posterior a trabalho mesh, sem provar qual comando causou o hang.

Na execução `configured-skip-msr-e135`, o filtro foi configurado para o shader mesh
`e135eaf39576263a`. Houve 1.794 linhas `first bind` e FPS até 32,6 ms/frame, mas a captura
terminou em `DXGI_ERROR_DEVICE_HUNG`. O log não contém `skipped shader e135...`; como o caminho
de draw mesh não registra a razão de descarte em todos os casos, não está confirmado que o
filtro tenha efetivamente removido esse shader. O DRED parou novamente num upload `16x4 f10`, na
lista 143, comando 406/458, depois de uma sequência semelhante de mesh draws e `ExecuteIndirect`.
O marcador de conclusão reporta a nota 126, logo a cópia indicada não deve ser tratada como
causa. As duas execuções mantiveram a tela branca segundo a observação do usuário; nenhuma é
sucesso visual.

**Próximo passo:** tornar observável o filtro de pipelines para mesh (contar/logar cada descarte
em `prepare_for`, sem alto volume) e então repetir uma captura com o filtro aplicado. Em paralelo,
correlacionar a criação e reutilização dos uploads `16x4 f10` com os registros de textura e
staging. Ainda não há causa raiz confirmada. O log desta rodada também informa 65 páginas zero
na biblioteca mapeada (304 MB); isso é uma anomalia medida já vista em outras rodadas, mas não há
evidência de que explique o hang da GPU.

### Supressão seletiva dos uploads 16x4 f10 (2026-11-27)

A build de diagnóstico foi recompilada com o filtro mesh registrando descartes em
`prepare_for`; a instalação local de `cargo-xwin` foi necessária, pois o script Bash não via o
`C:\Users\gilma\.cargo\bin`. O filtro de pipeline continuou sem evidência positiva nos shaders
mesh tentados: hashes vistos em capturas anteriores não reapareceram na janela DRED da rodada em
que foram configurados. Assim, essas rodadas de shader isolado permanecem inconclusivas.

Na captura `configured-skip-upload-16x4f10`, `VICTORY_SKIP_UPLOAD=16x4:10` confirmou 400
uploads suprimidos. O título continuou até 2.118 `first bind`, reportou FPS até 373,6 ms/frame
perto do fim e então recebeu `DXGI_ERROR_DEVICE_HUNG`. O DRED deixou de marcar o upload 16x4 f10
como comando parado e moveu-se para a lista 25, comando 65/111 (op 8 sem anotação). Os comandos
anteriores incluem cópias de textura, alias copies, `dispatch mesh` (`a26422c01dcf5612`),
`ExecuteIndirect` mesh (`c5ffa556ab926155`) e outro mesh (`ba8d52f4ea1a1bac`). A marca de
conclusão é 12, com as notas de alias copy e draws indiretos em seguida. Portanto, os uploads
seletivos não são necessários para o hang; o novo ponto DRED também não identifica o comando
causador.

A captura seguinte tentou pular `a26422c01dcf5612`, mas não registrou um descarte desse shader;
o DRED passou a parar mais tarde (lista 122, comando 487/587) num upload `16x4 f10`, com vários
`e135eaf39576263a` nos 40 comandos anteriores. A tela continuou branca. O contador do filtro não
é ainda prova de mudança causal: é preciso capturar os mesmos shaders na mesma trajetória ou
registrar o filtro por identidade de pipeline na submissão.

Conclusão até aqui: as cópias/alias copies e uploads `16x4 f10` isolados foram testados sem
remover o hang. A sequência mesh permanece uma hipótese de investigação, não uma causa raiz. O
próximo trabalho offline deve mapear os hashes mesh efetivamente presentes na última lista e
correlacionar seus recursos/estágios; não generalizar do comando que o DRED marcou.

### Bisseção mesh pelo sufixo (2026-11-27)

A listagem offline completa do `shaders.rpf` mapeou as variantes observadas na janela DRED. Entre
elas: `736b337d48bc0c57` = `MSNC_VS_TransformSkin_AlphaPrepass`; `c5ffa556ab926155` =
`MSNC_VS_TransformSkin`; `e135eaf39576263a` = `MS_VS_TransformAlphaPrepass_eb34a03f`;
`aa017fca6b1feb21` = `MS_VS_TransformAlphaPrepass`; `ba8d52f4ea1a1bac` =
`MS_VS_Transform_eb34a03f`; `a26422c01dcf5612` = `MSNC_VS_PedTransformSkinAlpha`. Os nomes
vêm da listagem `translate --list`, não apenas do rótulo DRED.

Na captura `configured-mesh-scissor`, `VICTORY_MESH_TEST=scissor` manteve os shaders mesh,
limitando a rasterização a um pixel. Ainda houve `DXGI_ERROR_DEVICE_HUNG` após 1.891 `first
bind`; o DRED marcou upload `16x4 f10` na lista 122, comando 596/696. Os comandos anotados nos
40 anteriores incluem `e135 / 8ba01719b71d1d35` repetidamente. Isso enfraquece rasterização de
uma grande cobertura como hipótese suficiente, mas não isola a execução do mesh de outros
trabalhos na lista.

A condição `VICTORY_MESH_TEST=keep:a:3` restringiu dispatches mesh a hashes terminados em `3a`
(um sufixo aproximado, não identidade completa). Na captura `configured-keep-mesh-e135-3a`, o
único par mesh/pixel nos 40 comandos antes do DRED foi
`e135eaf39576263a / 8ba01719b71d1d35`, correspondente a
`MS_VS_TransformAlphaPrepass_eb34a03f / PS_AlphaPrepass_eb34a03f`. Mesmo com esse único par mesh
visível na janela, houve `DXGI_ERROR_DEVICE_HUNG` após 1.798 binds. Isso prova que a combinação
controlada com esse par é suficiente para reproduzir o hang naquele fluxo restrito, não que o
mesh sozinho seja a causa.

Na contraprova `configured-keep3a-skipe135`, o mesmo `keep:a:3` foi combinado com
`VICTORY_SKIP_PIPELINES=e135eaf39576263a`. O log confirmou quedas atribuídas a `e135` (9 e depois
38 por janela de FPS), além de mesh descartados pelo filtro de sufixo. A execução passou 250 s
(4 min 56 s de log), chegou a 3.415 `first bind`, continuou reportando FPS (último registro
1,9 fps / 528,5 ms por frame; 374,5 ms de GPU) e não registrou `DXGI_ERROR_DEVICE_HUNG`. Foi
interrompida pelo limite da captura. Esta é evidência forte de que executar o par
`e135 / 8ba01719b71d1d35` participa do hang nessa trajetória restrita; não é prova isolada da
causa raiz, porque o filtro reduz drasticamente os demais draws e a tela fica sem imagem
utilizável.

Próximo passo offline: inspecionar o código nativo de `e135` e seu pixel correspondente (wave
size, uso de LDS, saídas mesh) e procurar uma validação/reprodução sintética desse par. Também
falta repetir a contraprova num percurso que mantenha o restante dos mesh ativos; as tentativas
anteriores em fluxo completo não registraram descartes de `e135`, então não servem como essa
confirmação.

### Confirmação cruzada da família AlphaPrepass wave64 (2026-11-27)

O host confirmou em log `WaveLaneCountMin=16, max=32`. O census dos quatro shaders do par
`e135 / 8ba01719b71d1d35` mostrou `wave64=1` para ambos. Os dois validaram no `dxil.dll` local
e tiveram pipeline criada pelo comando offline. O container original de `e135` tem 128 threads,
128 vértices/primitivas máximos e nenhum LDS declarado; seu census contém cinco wave ops. O
pixel tem 15 wave ops e loops. A criação de pipeline e a validação não exercitam a execução no
silício.

A segunda variante `aa017fca6b1feb21 / c2c7dc37d15fdcad`, também identificada no catálogo como
`MS_VS_TransformAlphaPrepass_eb34a03f / PS_AlphaPrepass_eb34a03f`, é outro par wave64. Ambos os
containers validaram e criaram pipeline no teste offline; seus censuses mostram `wave64=1`, 9
wave ops no mesh e 15 no pixel. Com `VICTORY_MESH_TEST=keep:1:2`, o DRED mostrou exatamente esse
par na janela dos 40 comandos anteriores e houve `DXGI_ERROR_DEVICE_HUNG` após 1.789 binds.
Assim, duas variantes de conteúdo distinto da mesma família reproduzem o hang quando mantidas
como os únicos mesh shaders daquela classe de sufixo.

A leitura do código explica por que esta é a hipótese principal: a detecção de lanes/ondas em
`emit/lower.rs` fica dentro de `if let StageIo::Cs(...)`; as tentativas de wave32 para shaders
wave64 em `emit/mod.rs` também só rodam para `HwStage::Cs`. No host, o intervalo medido é 16–32.
Logo, os estágios Ms/Ps wave64 não passam pela conversão que existe para compute, embora ainda
sejam traduzidos, validados e aceitos como PSO. A Arc só executa waves até 32. O padrão observado
é compatível com semântica wave64 de mesh/pixel emitida sem adaptação para esse host e travando
na execução do driver. O runtime não captura erro de validação antes do TDR.

**Conclusão atual, alta confiança:** a causa está na execução de pares AlphaPrepass mesh/pixel
wave64 na Arc B580 (host waves 16–32), não nos uploads 16x4 f10 que o DRED marca. Duas variantes
wave64 reproduzem o hang sob `VICTORY_MESH_TEST=keep`; com a variante e135 removida dentro desse
mesmo filtro, a execução passou 4:56 e 3.415 binds sem remoção do device. A prova ainda é de
causalidade em trajetórias filtradas, que não produzem imagem válida; não distingue se o defeito
final está na tradução wave64→wave32 dos estágios Ms/Ps, no DXIL que ela gera ou num bug do driver
Intel ao executar esse DXIL. Também não prova que esses sejam os únicos pares culpados na captura
completa. Não apliquei correção de shader nem alterei o jogo.

Próxima ação se a investigação continuar para além da localização: gerar uma alternativa wave32
correta para Ms/Ps (ou recusar esses shaders wave64 de forma explícita), repetir o controle
completo e verificar se o primeiro frame grande desenha corretamente. Não ligar uma aproximação
por padrão sem conferir a semântica dos wave ops e a imagem.

### Contraprova da segunda variante AlphaPrepass (2026-11-27)

Na execução `configured-keep12-skip-aa017`, foram combinados
`VICTORY_MESH_TEST=keep:1:2` e `VICTORY_SKIP_PIPELINES=aa017fca6b1feb21`. O filtro registrou
11 linhas de descartes de `aa017`; o log terminou após 3 min 17 s com 2.823 `first bind`, 34
linhas de FPS, FPS recente entre 0,8 e 1,1 e sem `DXGI_ERROR_DEVICE_HUNG`. O usuário informou
que fechou o jogo manualmente. A captura de controle `configured-keep-aa017-21`, com o mesmo
filtro mesh e `aa017` ativo, terminou em TDR após 1.789 binds e tinha o par
`aa017 / c2c7` na janela DRED.

Esse par passa a ter uma contraprova direta no fluxo filtrado: ativo reproduz o TDR; omitido,
o TDR não ocorre durante mais de 250 s, com o título continuando a reportar frames. A tela desse
filtro não é uma validação visual do jogo completo. Junto com a contraprova e135 já registrada,
o nexo com os pares wave64 AlphaPrepass fica forte; a captura completa ainda pode envolver outras
variantes da mesma família.

### Limite de wave ops em mesh (2026-11-27)

Leitura estática de `emit/lower.rs` mostra que mesh não herda o caminho de adaptação wave64
implementado para compute. O índice plano de mesh vira `flattenedThreadIdInGroup`; o índice
de lane lógico é `flat & (wave_size - 1)`. Porém, `cross_lane()` recusa operações entre lanes
em mesh porque SM 6.6 não permite declarar wave size para mesh. Reduções sem máscara só aceitam
min/max, explicitamente limitadas à wave física do host; soma, prefixo, operações de máscara e
outras reduções são recusadas. Isso afasta a hipótese de que o DXIL emitido esteja usando
diretamente uma wave DXIL de 64 lanes, mas deixa uma lacuna semântica possível: reduções min/max
de um shader wave64 são executadas por wave física (16–32 lanes), não pela wave lógica completa.

A correlação do hang com os pares AlphaPrepass continua sendo evidência de runtime; a captura
de `aa017` com pipeline omitida é censurada pelo fechamento manual do usuário e não deve ser
descrita como execução concluída sem hang.

O mapeamento confirma uma incompatibilidade mais específica nas duas variantes mesh. Os cinco
`v_readfirstlane_b32` de `e135` aparecem no DXIL como cinco `dx.op.waveReadLaneFirst.i32`. A
segunda variante, `aa017`, tem nove `v_readfirstlane_b32` nativos e o DXIL retransladado tem
nove `dx.op.waveReadLaneFirst.i32`; o pixel `c2c7` não contém essas chamadas. O lowering de mesh
chama `first_lane()`, que usa a wave física DXIL. O intercâmbio entre duas
metades de 32 lanes só é gerado quando `exchange` existe; essa adaptação é preparada dentro do
caminho de compute (`StageIo::Cs`). Portanto, se o operando varia entre lanes 0–31 e 32–63, as
duas metades do host leem valores diferentes, enquanto a wave64 do console escolhe um único
primeiro lane. Isso demonstra o mapeamento semântico incompatível no DXIL emitido, mas ainda
não prova que algum desses operandos difere entre as duas metades em runtime nem que a
divergência é a causa direta do TDR. A retranslação foi offline, wave máxima 32, sem validação
de pipeline ou desenho; não altera os resultados de runtime registrados acima.
