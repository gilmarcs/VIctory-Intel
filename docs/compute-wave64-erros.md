# Erros dos programas de compute wave64 (host de 32 lanes)

Medido com `./translate-waves.sh --validate` (run `run-20261126-161525`, `VICTORY_WAVE_LDS_WIDE=1`,
sem `--pipelines`, 10 threads). Os números são de programas de compute (`Cs`), não de contêineres.

## Resultado

| Situação | Programas |
|---|---|
| Compute com wave64 | 1155 |
| Convertidos para wave32 | 1144 (todos validados pelo DXC 1.9.2609) |
| Recusados: cruzamento de metades + LDS | 6 |
| Outra falha: descritor lido só como dado | 5 |

Antes de `VICTORY_WAVE_LDS_WIDE=1` havia 26 recusados (8 só de LDS e 18 com cruzamento de metades).
O interruptor converte os 8 só de LDS e 12 dos 18 com cruzamento.

## Os 6 recusados (cruzamento de metades + LDS)

Motivo: `cross-half code and group LDS without a barrier every thread reaches`.

| Shader | Hash |
|---|---|
| vfx_proj_decal_trail:CS_BuildProjDecalTrailProjectionBoxInstances_2342df81 | 3b02415f67c6b0d8 |
| vfx_proj_decal_trail:CS_BuildProjDecalTrailVertexBufferFromNodes_f859fb73 | 1d434307d981352a |
| vfx_proj_decal_trail:CS_DefragHeap_ComputeRelocations_9e6def81 | 22338086ea4c5364 |
| vfx_triangleClip:CS_DefragHeap_ComputeRelocations_9e6def81 | 02384a60fda6f2d0 |
| vfx_triangleClip:CS_CollectTriangles_cb481f38 | 6832a9da15c2e584 |
| vfx_triangleClip:CS_CollectTriangles_Meshlets_cb481f38 | 6b82bc7d22016cb5 |

Causa: o `widen` (ondas em nível de branch) resolve a região de LDS, mas só para grupos de 64 threads.
Estes grupos têm 128, 512 ou 1024 threads (várias ondas de console), e o `widen` recusa com
`wave-wide branches in a thread group of several console waves is not supported`.
O `CS_DefragHeap_ComputeRelocations` (`02384a60fda6f2d0`) é usado pelo jogo: aparece 13 vezes no log.

## Os 5 com outra falha

Motivo: `a descriptor read only as data` (não é de wave).

- shadowmask:CS_TraceShadowRays_Eighth_db2f6be8 (`96ccb14ef981fd85`)
- shadowmask:CS_TraceShadowRays_Quarter_2d98e5c6 (`0e64f14fae060d04`)
- shadowmask:CS_TraceShadowRays_Half_8988d8a5 (`002e93a6cc2aa355`)
- shadowmask:CS_TraceShadowRays_Full_457a0336 (`07c93bfb77fecebb`)
- shadowmask:CS_TraceShadowRays_FullAdaptive_ae7b9cf0 (`dbdbf36298727135`)

## Criação de pipeline (`translate --pipelines`): não resolvido

Não há resultado completo com `--pipelines`. As runs com essa opção pararam antes do fim.

- Sempre que a run chega perto de 22250 contêineres (de ~26500), as threads de tradução estouram a
  pilha. O estouro acontece logo depois do programa `CS_LaserLightShadowRaytracing_c3ab3042`
  (`#32ed072b9f845048`, `shaders.rpf@0xc075377`).
- Isolado (`VICTORY_TRANSLATE_ONLY=32ed072b9f845048`): sem `--pipelines` ele emite e valida; com
  `--pipelines` estoura logo depois do rastro. Aumentar a pilha para 256 MiB não muda nada, e a run
  fica presa até o `timeout`. Então não é falta de pilha: é recursão sem fim no caminho de criação de
  pipeline do `translate`.
- O jogo usa esse shader sem erro (`first bind` no frame 1618). O caminho do `translate` (`Host` em
  `crates/victory-shader/src/emit/host.rs`) é separado do caminho do jogo (`victory-gpu`).
- Uma run com `JOBS=10` e `--pipelines` também crashou com `double free` antes de emitir nada.

## Como reproduzir

```bash
# sem pipelines: a run completa
VICTORY_WAVE_LDS_WIDE=1 ./translate-waves.sh --no-build --validate

# o contêiner que estoura, isolado (com pipelines)
VICTORY_TRANSLATE_ONLY=32ed072b9f845048 VICTORY_TRANSLATE_TRACE=1 \
  <proton> runinprefix target/x86_64-pc-windows-msvc/diag/victory.exe translate \
  'Z:\...\shaders.rpf' --list --validate --pipelines --jobs 1
```
