# Ambiente local: Windows / VIctory

Este clone é independente de `../VIctory` e está sendo usado no Windows x64.
Os scripts locais executam o jogo por D3D12 nativo. Não presuma que correções
ou alterações locais do outro clone estão presentes.

## Compilar e iniciar

- Use o Rust fixado em `rust-toolchain.toml`. No PowerShell, `scripts\run-gta.ps1`
  compila e inicia o perfil release; `scripts\run-gta-diag.ps1` compila e inicia
  o perfil `diag` com `diagnostics` e a camada de debug D3D12. Ambos usam até
  10 jobs e aceitam variáveis `VICTORY_*` predefinidas no ambiente, que têm
  precedência sobre os valores padrão dos scripts. Para compilar sem iniciar:

  ```bash
  cargo build --locked --release -j 10 -p victory
  ```

- Os scripts esperam o jogo em `C:\GTA VI\GTA6.Xbox.v1.0.2.1-CYBERLEEK`.
  Edite `$gameDir` no script para usar outro local. Confirme que os arquivos do
  pacote terminaram de ser copiados ou baixados; um cabeçalho `9FPR` válido não
  prova a integridade de um `.rpf`. Não altere os arquivos do jogo.
- `scripts\run-gta.ps1` grava `run-gta.log` no perfil do usuário. Para iniciar
  com diagnósticos, use:

  ```bash
  .\scripts\run-gta-diag.ps1
  ```

- Os scripts limitam a janela a 1920x1080 e usam até 10 workers de pipeline.
  O script diagnóstico ativa debug D3D12, DRED e timing; deixa GBV desligado
  para evitar watchdog durante compute. Ambos enviam a saída para o arquivo de
  log no perfil do usuário; consulte o próprio script para todos os padrões.

## Diagnóstico

- Sem iniciar o jogo, `victory.exe translate --validate --list --jobs 8`
  recebe o caminho de `shaders.rpf`. Aponte `VICTORY_DXIL` para um `dxil.dll`
  x64 recente do DirectX Shader Compiler. Nesta revisão
  `VICTORY_HOST_MAX_WAVE=32` é lido pelo comando `translate`; não suponha que
  seja aplicado à execução do jogo.
- Distinga DXIL emitido, DXIL validado, pipeline criada e desenho verificado.
  `--pipelines` testa criação, não correção visual. A Arc expõe ondas de 16 a
  32 lanes; shaders Wave64 podem validar e ainda ser rejeitados pelo driver.
- Para casos mesh acima de 32 KiB, consulte
  `docs/estrategia-rastreamento-shaders.md` e valide por par mesh + pixel.
  Os documentos de rastreamento copiados do outro clone são contexto histórico:
  confirme caminhos, hashes e conclusões nesta revisão antes de aplicá-los.
- Mantenha logs e dados de teste fora do Git. Para testar um PR, use worktree
  isolada e `CARGO_TARGET_DIR` separado; compare a mesma base, arquivos do jogo,
  adaptador e validador. Não publique nem mescle sem pedido.

## Ondas wave64 → wave32 (compute)

- Para saber, sem abrir o jogo, o que o tradutor faz com cada programa de compute
  wave64 num host de 32 lanes (convertido, recusado ou outra falha), rode
  `./translate-waves.sh` no ambiente Linux configurado para a tradução
  (`--no-build` pula a compilação; `--rpf` aponta outro `shaders.rpf`). O script
  compila o `victory.exe` diagnóstico, traduz com `VICTORY_HOST_MAX_WAVE=32` e
  imprime o resumo. A saída fica no diretório de logs do script.
- `./translate-waves.sh --validate` também valida o DXIL com o `dxil.dll` do DirectX Shader
  Compiler v1.9.2609 (x64), em `~/.local/share/cool_fox/dxc/dxil.dll`. Ele não vem no Proton
  (procure `dxil*` em `files/lib/wine` e no prefixo: não existe). Para baixar, use o pacote
  `dxc_2026_09_29.zip` da release `v1.9.2609` do `microsoft/DirectXShaderCompiler`, confira o
  SHA-256 publicado pelo GitHub e extraia só `bin/x64/dxil.dll`. Outro caminho: `VICTORY_DXIL`.
- O validador só confirma o DXIL. Não prova que a Arc cria a pipeline nem que a imagem está
  correta: isso é `translate --pipelines` e o desenho no jogo.
- A conta vale só para compute (`Cs`). O flag `wave64` do census também vale para VS, PS e MS,
  onde não há essa conversão; por isso o script filtra por `Cs`.
- O census (`VICTORY_SHADER_CENSUS`) registra só programas emitidos. Recusados aparecem só em
  `list.txt`, com o sufixo ` at 0x…` no rótulo. Um programa recusado num jogo pode não aparecer
  como recusado no resumo, porque cada tentativa de tradução conta separadamente.
- Os logs de jogo (`WAVE64-WHY`, `WAVE64-CLAMP`, `WAVE64-TIME`, `WAVE64-RETRY`,
  `WAVE32-APPROXIMATE`) são por tentativa, não por programa final. Para o resultado final de um
  programa, use o resumo do `translate-waves.sh`.
- Recusas de LDS (`no wave32 lowering: … group LDS …`) são o caso a tratar pela barreira
  (`split_lds`); `VICTORY_WAVE_LDS_WIDE=1` só afeta os programas recusados só por LDS, e é opt-in.

## Recursos da máquina

A máquina tem 28 threads lógicas e 14 núcleos físicos. Use no máximo 10
threads por comando: por exemplo, `cargo build -j 10`, `--jobs 10`,
`cargo test -j 10 -- --test-threads=10` ou `make -j10`.
