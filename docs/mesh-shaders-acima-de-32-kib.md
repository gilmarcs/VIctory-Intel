# 71 mesh shaders com saída acima de 32 KiB

Inventário da execução de `victory translate --validate --list` sobre o pacote Xbox Series 1.0.2.1, usando a revisão `63bcca4` do VIctory como controle. O validador oficial da Microsoft rejeitou estes 71 DXILs isolados com `vertex plus primitive output size is greater than 32768`.

## Resultado da ligação (`translate_linked` sem leitura de pixel)

Traduzi os 71 mesh isolados e os liguei sem nenhuma saída de pixel lida (o mínimo possível). O resultado é 68 em 2048 bytes e 3 em 2560 bytes, contra o limite de 32768. Nenhum falhou.

Isso não prova que o jogo funciona: o tamanho final depende do pixel par. Cada escalar de saída custa cerca de 128 bytes (32 vértices × 4 bytes), então o pixel teria de ler perto de 240 escalares para estourar o limite. O par de cada mesh não foi identificado nesta execução; a validação de fato exige o par, e o jogo registra recusas com `no host pipeline` (uma vez por motivo). Nos logs disponíveis não há nenhuma recusa desse tipo.

Todos os offsets são relativos a `shaders.rpf` e identificam os registros no relatório. Nomes repetidos em offsets diferentes correspondem a registros diferentes; não devem ser agrupados só pelo nome. A lista conta programas, não passes ou pares mesh + pixel: um programa pode aparecer em mais de um passe.

Consulte a [estratégia de rastreamento](../VIctory/docs/estrategia-rastreamento-shaders.md) para investigar a produção e o consumo por componente. Este inventário é local e fica fora do repositório por conter nomes e offsets extraídos dos arquivos do jogo.

| Nº | Offset em `shaders.rpf` | Mesh shader | Investigação |
|---:|---|---|---|
| 1 | `0x101ec40` | `MS_VS_PedTransform_Decals_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 2 | `0x1024116` | `MS_VS_PedTransformFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 3 | `0x1025c70` | `MS_VS_PedTransformTransmissionFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 4 | `0x1032b0f` | `MSNC_VS_PedTransformSkin_Decals_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 5 | `0x103b6cd` | `MSNC_VS_PedTransformSkinFire_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 6 | `0x103e420` | `MSNC_VS_PedTransformSkinTransmissionFire_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 7 | `0x112ef09` | `MSNC_VS_PedTransformSkin_Decals_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 8 | `0x1139693` | `MSNC_VS_PedTransformSkinFire_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 9 | `0x113cf33` | `MSNC_VS_PedTransformSkinTransmissionFire_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 10 | `0x1158b22` | `MS_VS_PedTransform_Decals_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 11 | `0x115dfde` | `MS_VS_PedTransformFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 12 | `0x115fb36` | `MS_VS_PedTransformTransmissionFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 13 | `0x117004b` | `MSNC_VS_PedTransformSkin_Decals_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 14 | `0x117af9e` | `MSNC_VS_PedTransformSkinFire_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 15 | `0x117ead6` | `MSNC_VS_PedTransformSkinTransmissionFire_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 16 | `0x11a30ba` | `MS_VS_PedTransform_Decals_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 17 | `0x11a80b0` | `MS_VS_PedTransformFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 18 | `0x11a9ba5` | `MS_VS_PedTransformTransmissionFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 19 | `0x11c20e7` | `MSNC_VS_PedTransformSkin_Decals_6c7ff3fe_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 20 | `0x11d3249` | `MSNC_VS_PedTransformSkinFire_6c7ff3fe_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 21 | `0x11d8e5f` | `MSNC_VS_PedTransformSkinTransmissionFire_6c7ff3fe_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 22 | `0x163c1f8` | `MS_VS_PedTransform_Decals_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 23 | `0x16411fe` | `MS_VS_PedTransformFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 24 | `0x1642cf7` | `MS_VS_PedTransformTransmissionFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 25 | `0x165906e` | `MSNC_VS_PedTransformSkin_Decals_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 26 | `0x166885d` | `MSNC_VS_PedTransformSkinFire_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 27 | `0x166dc0c` | `MSNC_VS_PedTransformSkinTransmissionFire_70522ddb_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 28 | `0x169c353` | `MS_VS_PedTransform_Decals_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 29 | `0x16a1333` | `MS_VS_PedTransformFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 30 | `0x16a2e22` | `MS_VS_PedTransformTransmissionFire_52f61330_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 31 | `0x16bb316` | `MSNC_VS_PedTransformSkin_Decals_6c7ff3fe_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 32 | `0x16cc455` | `MSNC_VS_PedTransformSkinFire_6c7ff3fe_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 33 | `0x16d2060` | `MSNC_VS_PedTransformSkinTransmissionFire_6c7ff3fe_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 34 | `0x3cd861d` | `MS_VS_TransformD_7095f810_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 35 | `0x3cdbb35` | `MS_VS_MirrorReflectionTransformD_ed3bd86e_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 36 | `0x3cde7dc` | `MSR_VS_TransformSkinD_34f92402_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 37 | `0x3ce0737` | `MS_VS_TransformSkinD_34f92402_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 38 | `0x3ce682b` | `MSR_VS_TransformSkinD_NoDeform_34f92402_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 39 | `0x3ce87a1` | `MS_VS_TransformSkinD_NoDeform_34f92402_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 40 | `0x3cee8e9` | `MSR_VS_MirrorReflectionTransformSkinD_07a69031_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 41 | `0x3cf0959` | `MS_VS_MirrorReflectionTransformSkinD_07a69031_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 42 | `0x3cfb54a` | `MS_VS_Transform_d69a8879_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 43 | `0x3cfe246` | `MSR_VS_TransformSkin_f99c720c_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 44 | `0x3d001b3` | `MS_VS_TransformSkin_f99c720c_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 45 | `0x40f671a` | `MS_VS_TransformSpatter_Fire_AlphaPrepass_eb34a03f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 46 | `0x412ffb0` | `MSR_VS_TransformSkinSpatter_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 47 | `0x4132de8` | `MS_VS_TransformSkinSpatter_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 48 | `0x4165864` | `MS_VS_TransformSpatter_Fire_AlphaPrepass_eb34a03f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 49 | `0x41a8703` | `MSR_VS_TransformSkinSpatter_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 50 | `0x41abea8` | `MS_VS_TransformSkinSpatter_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 51 | `0x428a11c` | `MS_VS_TransformSpatter_Fire_AlphaPrepass_eb34a03f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 52 | `0x42c4b5d` | `MSR_VS_TransformSkinSpatter_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 53 | `0x42c7aa0` | `MS_VS_TransformSkinSpatter_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 54 | `0x4307304` | `MS_VS_Transform_Fire_AlphaPrepass_6f3bc7d5_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 55 | `0x430b359` | `MS_VS_TransformTransmissionSpatterFire_6f3bc7d5_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 56 | `0x430f6ed` | `MS_VS_TransformSpatter_Fire_AlphaPrepass_6f3bc7d5_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 57 | `0x435388e` | `MSR_VS_TransformSkin_Fire_AlphaPrepass_2abd5519_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 58 | `0x4359141` | `MS_VS_TransformSkin_Fire_AlphaPrepass_2abd5519_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 59 | `0x435bfcd` | `MSR_VS_TransformSkinSpatterTransmissionFire_2abd5519_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 60 | `0x4361608` | `MS_VS_TransformSkinSpatterTransmissionFire_2abd5519_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 61 | `0x43642d6` | `MSR_VS_TransformSkinSpatter_Fire_AlphaPrepass_2abd5519_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 62 | `0x4369d2e` | `MS_VS_TransformSkinSpatter_Fire_AlphaPrepass_2abd5519_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 63 | `0x43a4472` | `MS_VS_Transform_Fire_AlphaPrepass_eb34a03f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 64 | `0x43a65b3` | `MS_VS_TransformTransmissionSpatterFire_eb34a03f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 65 | `0x43a87ae` | `MS_VS_TransformSpatter_Fire_AlphaPrepass_eb34a03f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 66 | `0x43d7340` | `MSR_VS_TransformSkin_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 67 | `0x43da139` | `MS_VS_TransformSkin_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 68 | `0x43dd165` | `MSR_VS_TransformSkinSpatterTransmissionFire_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 69 | `0x43dfe96` | `MS_VS_TransformSkinSpatterTransmissionFire_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 70 | `0x43e2d31` | `MSR_VS_TransformSkinSpatter_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
| 71 | `0x43e5bf5` | `MS_VS_TransformSkinSpatter_Fire_AlphaPrepass_dc8b9b7f_Wrapped` | Cabe ligado sem leitura de pixel (ver acima) |
