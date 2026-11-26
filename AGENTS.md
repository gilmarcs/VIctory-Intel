# Ambiente local: cool_fox / VIctory

Este clone é independente de `../VIctory`. O alvo é Windows x64, executado
no Linux com Steam, Proton Experimental e Intel Arc B580 (Mesa/ANV).
Não presuma que correções ou alterações locais do outro clone estão presentes.

## Compilar e iniciar

- Use o Rust fixado em `rust-toolchain.toml` e `cargo-xwin`. Para uma build de
  diagnóstico, execute `./build.sh` (perfil `diag`, feature `diagnostics`,
  alvo `x86_64-pc-windows-msvc`, até 10 jobs). Não substitua por build nativa
  Linux. A versão release pode ser compilada com:

  ```bash
  "$HOME/.cargo/bin/cargo-xwin" xwin build --locked --release -j 10 \
    -p victory --target x86_64-pc-windows-msvc
  ```

- Confirme que os arquivos do pacote terminaram de ser copiados ou baixados;
  um cabeçalho `9FPR` válido não prova a integridade de um `.rpf`. Não altere
  os arquivos do jogo. Inicie a build de diagnóstico com:

  ```bash
  GAME_DIR='/run/media/gilmar/8CEE31B4EE319802/GTA VI/GTA6.Xbox.v1.0.2.1-CYBERLEEK'
  ./run-proton.sh "$GAME_DIR/game_scarlett_master.exe"
  ```

- `VICTORY_EXE` seleciona outra build; `PROTON_PATH` e `STEAM_DIR` selecionam
  instalações alternativas. O prefixo padrão é
  `${XDG_DATA_HOME:-$HOME/.local/share}/cool_fox/proton`, separado do outro
  clone; `STEAM_COMPAT_DATA_PATH` o substitui. Não instale DLLs no prefixo
  como primeira tentativa.
- O launcher prioriza DXGI do DXVK e D3D12 do vkd3d-proton apenas na execução,
  define `VKD3D_DEBUG=info`, limita a janela a 1920x1080 e usa até 10 workers
  de pipeline. Também ativa diagnósticos e timing; as variáveis podem ser
  sobrescritas no ambiente. Consulte o script para detalhes.
- Logs ficam em `${XDG_STATE_HOME:-$HOME/.local/state}/cool_fox/logs/run`;
  `latest.log` aponta para o último. `VICTORY_LOG_DIR` muda o diretório e
  `PROTON_LOG=0` desliga o log adicional do Proton.

## Diagnóstico

- Para `proton runinprefix` direto, configure `STEAM_COMPAT_CLIENT_INSTALL_PATH`
  e `STEAM_COMPAT_DATA_PATH` como no launcher e acrescente
  `WINEDLLOVERRIDES='d3d12,d3d12core=n,b'`, preservando outros overrides.
  Para swap chain D3D12 use também `dxgi=n,b`. `WINEDEBUG=+loaddll` ajuda a
  confirmar as DLLs carregadas. Ausência de WARP não significa ausência de DX12.
- Sem iniciar o jogo, `victory.exe translate --validate --list --jobs 8`
  recebe um caminho Windows para `shaders.rpf`. Aponte `VICTORY_DXIL` para
  um `dxil.dll` x64 recente do DirectX Shader Compiler, também em caminho
  Windows. Nesta revisão `VICTORY_HOST_MAX_WAVE=32` é lido pelo comando
  `translate`; não suponha que seja aplicado à execução do jogo.
- Distinga DXIL emitido, DXIL validado, pipeline criada e desenho verificado.
  `--pipelines` testa criação, não correção visual. A Arc expõe ondas de 16 a
  32 lanes; shaders Wave64 podem validar e ainda ser rejeitados pelo driver.
- Para casos mesh acima de 32 KiB, consulte
  `docs/estrategia-rastreamento-shaders.md` e valide por par mesh + pixel.
  Os documentos de rastreamento copiados do outro clone são contexto histórico:
  confirme caminhos, hashes e conclusões nesta revisão antes de aplicá-los.
- Mantenha logs e dados de teste fora do Git, por exemplo em `/tmp/opencode`.
  Para testar um PR, use worktree isolada e `CARGO_TARGET_DIR` separado; compare
  a mesma base, Proton, arquivos e validador. Não publique nem mescle sem pedido.

## Ondas wave64 → wave32 (compute)

- Para saber, sem abrir o jogo, o que o tradutor faz com cada programa de compute
  wave64 num host de 32 lanes (convertido, recusado ou outra falha), rode
  `./translate-waves.sh` (`--no-build` pula a compilação; `--rpf` aponta outro
  `shaders.rpf`). Ele compila o `victory.exe` de diagnóstico, traduz o `shaders.rpf` pelo Proton
  com `VICTORY_HOST_MAX_WAVE=32` e imprime o resumo. A saída fica em
  `${XDG_STATE_HOME:-$HOME/.local/state}/cool_fox/logs/translate/latest/`.
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
