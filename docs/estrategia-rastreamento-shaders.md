# Estratégia para rastrear o fluxo de dados entre mesh e pixel shaders

O objetivo é descobrir **como o mesh shader produz cada componente de saída** e **como o pixel shader usa cada componente de entrada**. Analise os dois estágios separadamente e cruze os resultados por passe: um mesmo shader pode participar de vários pares.

## 0. Preparar os dois shaders para inspeção

1. Escolha **um offset** do [inventário de investigação](mesh-shaders-acima-de-32-kib.md). Rode `victory.exe shader-info --effects <caminho Windows para shaders.rpf>` pelo Proton, conforme [`AGENTS.md`](../AGENTS.md), e salve a saída fora do Git. Localize o passe e anote os hashes de contêiner e do motor para os dois estágios. Um mesh shader pode aparecer na coluna `vertex` do passe: confirme que o programa nativo é `Ms`. Se o nome se repetir, resolva pelo offset e pelos hashes, não pelo nome.
2. Para extrair só o par, leia o passe com [`sgd5::databases`](../crates/victory-game/src/sgd5.rs), resolva os índices de `pass.shaders` e use `record.unpack()` nos dois registros antes de chamar `victory_shader::emit::translate_with`. O offset de um registro SGD5 aponta para o registro empacotado, não para um DXBC pronto para copiar. Como alternativa em lote, use `translate --list` com `VICTORY_EMIT_DUMP` apontando para uma pasta **já criada**, fora do repositório. Essa alternativa gera muitos arquivos e pode deduplicar registros equivalentes; não identifique variantes só pelo nome do dump.
3. Guarde o DXIL **isolado** de cada estágio antes de fazer qualquer corte. Desmonte os dois arquivos com o `dxc` do DirectX Shader Compiler. Os comandos abaixo usam o `dxc` nativo do Linux; com `dxc.exe` use o Proton e passe caminhos Windows.

   ```bash
   cd /caminho/fora/do/repositorio
   dxc -dumpbin mesh.dxil > mesh.ll
   dxc -dumpbin pixel.dxil > pixel.ll
   rg -n 'store(Vertex|Primitive)Output|emitIndices|setMeshOutputCounts' mesh.ll
   rg -n 'loadInput|sample|textureLoad|discard|storeOutput|br i1' pixel.ll
   ```

   Se só houver um log de assinaturas, registre a máscara `Used`, mas **não** marque origem ou destino como confirmados: essas respostas precisam dos corpos dos shaders. Se o `dxc` não abrir o contêiner, registre o erro e inspecione a representação intermediária ou o código nativo Xbox; não substitua o shader real por um *stub* para fazer o rastreamento.

### Quando `shader-info` mostra `no parsable DXIL program`

O [`shader-info`](../apps/victory/src/shader.rs) usa a mesma mensagem tanto para uma parte `DXIL` ausente quanto para um erro de `DxilProgram::parse`. O relatório de código nativo é independente: uma linha que também mostra `native Ms` indica que o programa mesh nativo foi encontrado, não que a tradução falhou.

Nos itens **36 e 37** do inventário local, os dois contêineres desempacotaram e tinham uma parte `DXIL` com `XDAT`. O cabeçalho apontava para apenas quatro bytes no fim da parte, sem a assinatura de bitcode LLVM. A leitura do programa DXIL original parou em [`bitcode lacks LLVM magic`](../crates/victory-shader/src/dxil.rs); já o código nativo em `XDAT` pôde ser lido. Para esse caso, use `shader-info --disasm` no contêiner desempacotado para listar as instruções nativas, ou gere DXIL de PC com [`emit::translate_with`](../crates/victory-shader/src/emit/mod.rs), que traduz o programa nativo sem depender do bitcode original.

O teste isolado com `translate --validate --list` pelo Proton **emitiu DXIL para os dois**. O validador oficial rejeitou ambos pelo limite de 32 KiB de saída combinada de vértices e primitivas, não pelo erro de bitcode do contêiner Xbox. Nenhuma pipeline foi criada.

A associação aos passes foi confirmada cruzando o **hash do motor e o hash do contêiner** dos registros desempacotados com `shader-info --effects`. Os nomes se repetem em outros registros; cada um destes dois aparece em seis passes, com o mesmo conjunto de seis pixel shaders. Ainda falta rastrear a produção e o consumo por componente e validar o DXIL vinculado de cada par. Os identificadores e logs ficam no relatório local, fora do repositório.

## 1. Mesh shader: rastrear a produção

Para cada componente exportado, como `TEXCOORD1.w`, siga o valor **para trás**:

1. Na saída DXIL, localize `dx.op.storeVertexOutput` ou `dx.op.storePrimitiveOutput`. Resolva o atributo pela assinatura de saída (`OSG1` para vértices, `PSG1` para primitivas); o número do registrador não é necessariamente o índice de `TEXCOORD`.
2. Siga as definições do valor até as origens: entradas, buffers, constantes ou cálculos intermediários. Se caminhos diferentes produzem valores diferentes, registre cada origem e a condição correspondente.
3. Quando precisar comparar com o código do Xbox, use [`io.rs`](../crates/victory-shader/src/io.rs) como ponto de partida para relacionar as exportações nativas às saídas que o VIctory gera.

Para cada escrita, anote o ID do elemento de assinatura, linha, coluna e índice de vértice ou primitiva. Siga todos os valores de entrada de `phi` e os dois braços de `select` quando puderem alcançar a escrita. Se a origem for um buffer, registre também qual descritor e endereço foram identificados; não invente o significado se o endereço for dinâmico ou desconhecido.

**Resultado esperado:** para cada componente, o cálculo que o produz e os dados dos quais ele depende.

## 2. Pixel shader: rastrear o consumo

Para cada entrada marcada como `Used` na assinatura DXIL (`ISG1`), siga o valor **para a frente** a partir de `dx.op.loadInput`:

1. Se o valor chega a uma amostragem de textura, identifique se ele é usado como coordenada, nível de detalhe, gradiente ou outro argumento.
2. Se chega a uma comparação, verifique se ela controla um ramo ou um `discard`. Se chega a uma saída, registre se afeta a cor, a profundidade ou outro alvo. Um componente pode ter vários usos.
3. Registre leituras condicionais e índices dinâmicos. **Uma leitura não resolvida não torna o componente descartável.**

Mapeie os argumentos do `loadInput` à linha e ao canal da assinatura e siga **todos** os usos do valor SSA retornado. Confira também acessos por avaliação de atributos ou interpolação, quando presentes; não use a ausência literal de `loadInput` como única prova de que um canal não é lido. Registre a cadeia curta, por exemplo: `entrada → cálculo → coordenada de sample → saída de cor`. Essa cadeia é um modelo para o relatório, não uma conclusão sobre o caso real.

**Resultado esperado:** para cada componente, as operações que o consomem e as saídas ou decisões que ele pode afetar.

## 3. Cruzar por passe e validar

Use `victory.exe shader-info --effects shaders.rpf` para identificar os pares de cada passe. Cruze os componentes por semântica, índice, canal e tipo de saída (vértice ou primitiva), não só pelo número do registrador.

| Campo | O que registrar |
|---|---|
| Passe | Efeito, técnica, número do passe e hashes dos dois shaders |
| Atributo | Semântica, índice e componente, por exemplo `TEXCOORD1.w` |
| Produção | Origem, operações relevantes e condições no mesh shader |
| Consumo | Leituras, operações relevantes e efeitos no pixel shader |
| Evidência | Instruções ou valores usados na análise; estado confirmado, inferido ou não resolvido |

Não deduza o significado do nome: `TEXCOORD` pode transportar UVs, normais ou outros valores. Só classifique um componente como “UV”, por exemplo, quando o fluxo de dados sustentar essa conclusão.

### Primeiro caso: `alphaTransmissionfire_drawmeshlets`

No efeito `ped_standard_lod_muv_alpha`, passe 0, comece com dois controles:

- **`TEXCOORD1.w` é lido:** descubra quem o produz no mesh shader e quais operações o consomem no pixel shader.
- **`TEXCOORD8.xy` não é lido por esse pixel shader:** confirme a ausência de leitura e verifique que o atributo é removido na vinculação.

### Verificar o resultado no validador e na GPU

1. Em uma build isolada do [PR #2398](https://github.com/Marvisak/VIctory/pull/2398), passe os contêineres originais do par para `victory_shader::emit::translate_linked`. Compare as assinaturas DXIL antes e depois. Verifique que os canais lidos e as saídas de sistema necessárias continuam presentes. Use `emit::mesh_output_bytes` para comparar os tamanhos declarados e valide **ambos** os DXILs vinculados com o `dxil.dll` oficial. Para o par real já medido, a referência é **33 KiB isolado → 25 KiB vinculado**, com os dois estágios vinculados aceitos.
2. Para verificar o caminho de desenho pelo Proton neste ambiente, configure `WINEDLLOVERRIDES='d3d12,d3d12core=n,b'` como no [`run-proton.sh`](../run-proton.sh). Na build do #2398, rode o teste `linked_mesh_and_pixel_prune_unused_outputs_under_32k_and_draw` com `--nocapture` e confira que o caminho **hardware** não foi pulado. Na Arc B580, esse teste sintético já desenhou e conferiu os pixels com o override; WARP foi pulado.
3. Registre separadamente **validação DXIL do par real**, **criação da pipeline do par real** e **desenho com pixels verificados**. O teste sintético não fornece os recursos do passe real. `translate --pipelines` testa shaders isolados (pode usar um estágio substituto), e `VICTORY_PAIRS` monta um par a partir de DXILs já emitidos; nenhum desses caminhos, por si só, comprova a poda de `translate_linked`. Se a Arc recusar a pipeline por Wave64, registre a limitação do dispositivo separadamente.

### Resultado do item 1 do inventário

A poda foi aplicada ao DXIL de teste na árvore isolada do PR #2398, não na branch de trabalho `intel-gpu` nem nos arquivos do jogo. Nos dois pares distintos, a saída declarada do mesh caiu de **33 KiB para 28 KiB** (−5 KiB). O validador rejeitou o mesh isolado por exceder 32 KiB e aceitou os dois estágios vinculados de cada par.

Em ambos os pares, `TEXCOORD1.w` foi lido pelo pixel shader e preservado na vinculação. `TEXCOORD8.xy` aparecia na interface isolada sem uso indicado no pixel e foi removido das assinaturas vinculadas. O rastreamento de `TEXCOORD1.w` ainda não resolve a origem do valor no mesh nem seu efeito final no pixel.

**Criação de pipeline confirmada para os dois pares** na Intel Arc B580 com Proton Experimental/vkd3d-proton, sem WARP e sem shader substituto. O teste usou os DXILs vinculados dos shaders reais e a configuração de pipeline do `Host::mesh_pipeline`, não o estado completo do passe do jogo. Não houve desenho nem comparação de pixels.

Os identificadores dos passes, offsets e evidências ficam no relatório local, fora do repositório, conforme a [política de assets](asset-policy.md).

## 4. Ampliar para o conjunto

Depois do primeiro caso, automatize a análise para os passes associados aos **71 mesh shaders que excederam 32 KiB**. Reaproveite o rastreamento de cada shader pelo hash, mas calcule a economia e valide **por par**. Ordene o relatório por bytes economizados, pares que ainda excedem o limite e leituras não resolvidas.
