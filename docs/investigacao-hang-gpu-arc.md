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
