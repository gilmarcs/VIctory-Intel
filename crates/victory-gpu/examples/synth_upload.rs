//! A synthetic version of the texture upload path: many copies from an upload buffer into
//! placed textures, in command lists with resource barriers. By default this uses the small
//! 16x4, R16G16B16A16_FLOAT texture; `SYNTH_TEXTURE=WxH:FORMAT` selects another shape, and
//! `SYNTH_COPIES_PER_LIST` selects the number of copies per command list.
//!
//! `cargo run -p victory-gpu --example synth_upload -- <scenario> [iterations]`
//!
//! Scenarios: `committed` (each texture a committed resource), `placed64` (placed in a heap
//! at 64 KiB), `placed4` (placed at 4 KiB where the device allows), `churn` (placed at 4 KiB,
//! slots reused by textures of other shapes, with and without aliasing barriers).
//! A hang is a device removal or a fence not reached in 5 s.
#![cfg(windows)]
#![allow(clippy::undocumented_unsafe_blocks, unsafe_op_in_unsafe_fn, missing_docs)]

use std::mem::ManuallyDrop;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_12_0;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::Interface;

const COPIES_PER_LIST: u32 = 400;
const BLOCK: u64 = 64 << 20;

struct Gpu {
    device: ID3D12Device,
    queue: ID3D12CommandQueue,
    fence: ID3D12Fence,
    event: HANDLE,
    value: u64,
}

impl Gpu {
    /// Runs the lists and waits; `Err` with a reason if the device is lost or the wait times out.
    unsafe fn run(&mut self, list: &ID3D12GraphicsCommandList) -> Result<(), String> {
        self.queue.ExecuteCommandLists(&[Some(list.cast().unwrap())]);
        self.value += 1;
        self.queue.Signal(&self.fence, self.value).map_err(|e| format!("signal: {e}"))?;
        self.fence.SetEventOnCompletion(self.value, self.event).map_err(|e| format!("event: {e}"))?;
        let waited = WaitForSingleObject(self.event, 5000);
        if waited.0 != 0 {
            let reason = self.device.GetDeviceRemovedReason();
            return Err(format!("not done in 5 s (device removed reason {reason:?})"));
        }
        let reason = self.device.GetDeviceRemovedReason();
        if reason.is_err() {
            return Err(format!("device removed: {reason:?}"));
        }
        Ok(())
    }
}

fn buffer_desc(width: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: width,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        ..Default::default()
    }
}

fn texture_desc(width: u64, height: u32, format: DXGI_FORMAT, alignment: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: alignment,
        Width: width,
        Height: height,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        ..Default::default()
    }
}

unsafe fn adapter_device() -> (ID3D12Device, String) {
    let factory: IDXGIFactory1 = CreateDXGIFactory1().unwrap();
    let mut best = None;
    let mut i = 0;
    while let Ok(a) = factory.EnumAdapters1(i) {
        i += 1;
        let desc = a.GetDesc1().unwrap();
        let name = String::from_utf16_lossy(&desc.Description).trim_end_matches('\0').to_owned();
        eprintln!("adapter {i}: {name}");
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 == 0 && (best.is_none() || name.contains("Arc")) {
            best = Some((a, name));
        }
    }
    let (adapter, name) = best.expect("no hardware adapter");
    let mut device: Option<ID3D12Device> = None;
    D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_12_0, &mut device).unwrap();
    (device.unwrap(), name)
}

/// Textures that stay: a fixed set of small textures regrown on each round (shader-resource
/// to copy destination, a copy, back), then read by a compute dispatch that samples all of
/// them, many rounds to a list: what the game does with a texture it updates every frame.
#[allow(clippy::too_many_arguments)]
unsafe fn persist(
    gpu: &mut Gpu,
    device: &ID3D12Device,
    allocator: &ID3D12CommandAllocator,
    list: &ID3D12GraphicsCommandList,
    upload: &ID3D12Resource,
    heap: &ID3D12Heap,
    slot: u64,
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
    lists: u32,
) {
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::ID3DBlob;
    use windows::core::{PCSTR, s};
    const N: usize = 600;
    const ROUNDS: u32 = 6;
    let source = format!(
        "#define RS \"DescriptorTable(SRV(t0, numDescriptors={N})), DescriptorTable(UAV(u0, numDescriptors=1))\"\n\
         Texture2D<float4> tex[{N}] : register(t0);\n\
         RWStructuredBuffer<float4> outb : register(u0);\n\
         [RootSignature(RS)]\n\
         [numthreads(64,1,1)]\n\
         void main(uint3 id : SV_DispatchThreadID) {{\n\
           float4 s = 0;\n\
           for (uint i = 0; i < {N}; i++) s += tex[i].Load(int3(id.x & 3, id.x & 3, 0));\n\
           outb[id.x] = s;\n\
         }}\n"
    );
    let (mut code, mut errors): (Option<ID3DBlob>, Option<ID3DBlob>) = (None, None);
    let compiled = D3DCompile(
        source.as_ptr().cast(),
        source.len(),
        None,
        None,
        None,
        s!("main"),
        PCSTR(c"cs_5_1".as_ptr().cast()),
        1 << 20,
        0,
        &mut code,
        Some(&mut errors),
    );
    if compiled.is_err() {
        let e = errors.unwrap();
        panic!("{}", String::from_utf8_lossy(std::slice::from_raw_parts(e.GetBufferPointer().cast::<u8>(), e.GetBufferSize())));
    }
    let code = code.unwrap();
    let bytes = std::slice::from_raw_parts(code.GetBufferPointer().cast::<u8>(), code.GetBufferSize());
    let root: ID3D12RootSignature = device.CreateRootSignature(0, bytes).unwrap();
    let pso: ID3D12PipelineState = device
        .CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(&root),
            CS: D3D12_SHADER_BYTECODE { pShaderBytecode: bytes.as_ptr().cast(), BytecodeLength: bytes.len() },
            ..Default::default()
        })
        .unwrap();

    // The textures, one in each slot, and the views of them.
    let views: ID3D12DescriptorHeap = device
        .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            NumDescriptors: N as u32 + 1,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            NodeMask: 0,
        })
        .unwrap();
    let step = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
    let start = views.GetCPUDescriptorHandleForHeapStart();
    let small = slot < 65536;
    let alignment = if small { u64::from(D3D12_SMALL_RESOURCE_PLACEMENT_ALIGNMENT) } else { 0 };
    let mut textures: Vec<ID3D12Resource> = Vec::new();
    for i in 0..N {
        let mut desc = texture_desc(16, 4, DXGI_FORMAT_R16G16B16A16_FLOAT, alignment);
        let info = device.GetResourceAllocationInfo(0, &[desc]);
        if info.SizeInBytes == u64::MAX || (small && info.Alignment != alignment) {
            desc.Alignment = 0;
        }
        let mut t: Option<ID3D12Resource> = None;
        device
            .CreatePlacedResource(heap, i as u64 * slot, &desc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut t)
            .unwrap();
        let t = t.unwrap();
        let view = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: 0x1688,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 },
            },
        };
        device.CreateShaderResourceView(&t, Some(&view), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + i * step });
        textures.push(t);
    }
    let mut out: Option<ID3D12Resource> = None;
    device
        .CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() },
            D3D12_HEAP_FLAG_NONE,
            &D3D12_RESOURCE_DESC { Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, ..buffer_desc(64 * 16) },
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            None,
            &mut out,
        )
        .unwrap();
    let out = out.unwrap();
    device.CreateUnorderedAccessView(
        &out,
        None::<&ID3D12Resource>,
        Some(&D3D12_UNORDERED_ACCESS_VIEW_DESC {
            Format: DXGI_FORMAT_UNKNOWN,
            ViewDimension: D3D12_UAV_DIMENSION_BUFFER,
            Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                Buffer: D3D12_BUFFER_UAV { FirstElement: 0, NumElements: 64, StructureByteStride: 16, ..Default::default() },
            },
        }),
        D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + N * step },
    );
    let gpu_start = views.GetGPUDescriptorHandleForHeapStart();
    let uav_table = D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (N * step) as u64 };

    let barriers = |before, after| -> Vec<D3D12_RESOURCE_BARRIER> {
        textures
            .iter()
            .map(|t| D3D12_RESOURCE_BARRIER {
                Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                Anonymous: D3D12_RESOURCE_BARRIER_0 {
                    Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: std::mem::transmute_copy(t),
                        Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                        StateBefore: before,
                        StateAfter: after,
                    }),
                },
            })
            .collect()
    };
    let to_copy = barriers(D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST);
    let to_read = barriers(D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
    let uav_barrier = D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_UAV,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            UAV: ManuallyDrop::new(D3D12_RESOURCE_UAV_BARRIER { pResource: std::mem::transmute_copy(&out) }),
        },
    };
    let mut seed = 0x2545f4914f6cdd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    eprintln!("persist: {N} textures, {ROUNDS} rounds a list");
    for iteration in 0..lists {
        allocator.Reset().unwrap();
        list.Reset(allocator, None).unwrap();
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetComputeRootSignature(&root);
        list.SetPipelineState(&pso);
        list.SetComputeRootDescriptorTable(0, gpu_start);
        list.SetComputeRootDescriptorTable(1, uav_table);
        for round in 0..ROUNDS {
            // (the first round of the first list finds them as made: copy destinations)
            if iteration > 0 || round > 0 {
                list.ResourceBarrier(&to_copy);
            }
            for t in &textures {
                let at = (next() % (BLOCK / 512 - 4)) * 512;
                let to = D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(t),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
                };
                let from = D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(upload),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT { Offset: at, ..footprint },
                    },
                };
                list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
            }
            list.ResourceBarrier(&to_read);
            list.Dispatch(8, 1, 1);
            list.ResourceBarrier(&[uav_barrier.clone()]);
        }
        list.Close().unwrap();
        if let Err(why) = gpu.run(list) {
            println!("persist: HANG at list {iteration}: {why}");
            std::process::exit(2);
        }
        if iteration % 100 == 0 {
            eprintln!("persist: {iteration} lists done");
        }
    }
    println!("persist: OK, {lists} lists of {ROUNDS} rounds of {N} copies and reads");
}

/// The game's cross-format alias path: texture -> buffer -> texture.
unsafe fn alias_copy(
    gpu: &mut Gpu,
    device: &ID3D12Device,
    allocator: &ID3D12CommandAllocator,
    list: &ID3D12GraphicsCommandList,
    upload: &ID3D12Resource,
    width: u64,
    height: u32,
    source_format: DXGI_FORMAT,
    destination_format: DXGI_FORMAT,
    iterations: u32,
    copies_per_list: u32,
) {
    let source_desc = texture_desc(width, height, source_format, 0);
    let destination_desc = texture_desc(width, height, destination_format, 0);
    let mut source_footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
    let mut destination_footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
    let (mut source_rows, mut destination_rows) = (0u32, 0u32);
    let (mut source_bytes, mut destination_bytes) = (0u64, 0u64);
    device.GetCopyableFootprints(&source_desc, 0, 1, 0, Some(&mut source_footprint), Some(&mut source_rows), None, Some(&mut source_bytes));
    device.GetCopyableFootprints(&destination_desc, 0, 1, 0, Some(&mut destination_footprint), Some(&mut destination_rows), None, Some(&mut destination_bytes));
    let a = source_footprint.Footprint;
    let b = destination_footprint.Footprint;
    if source_bytes != destination_bytes
        || source_footprint.Offset != destination_footprint.Offset
        || source_rows != destination_rows
        || (a.Width, a.Height, a.Depth, a.RowPitch) != (b.Width, b.Height, b.Depth, b.RowPitch)
    {
        panic!("alias footprints differ: source {source_footprint:?} rows={source_rows} bytes={source_bytes}; destination {destination_footprint:?} rows={destination_rows} bytes={destination_bytes}");
    }
    eprintln!("alias footprints match: {}x{} f{} -> f{}, pitch {} rows {} total {}", width, height, source_format.0, destination_format.0, a.RowPitch, source_rows, source_bytes);

    let source_allocation = device.GetResourceAllocationInfo(0, &[source_desc]);
    let destination_allocation = device.GetResourceAllocationInfo(0, &[destination_desc]);
    let destination_offset = source_allocation.SizeInBytes.next_multiple_of(destination_allocation.Alignment);
    let mut heap: Option<ID3D12Heap> = None;
    device.CreateHeap(&D3D12_HEAP_DESC {
        SizeInBytes: destination_offset + destination_allocation.SizeInBytes,
        Properties: D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() },
        Alignment: 65536,
        Flags: D3D12_HEAP_FLAG_ALLOW_ONLY_NON_RT_DS_TEXTURES,
        ..Default::default()
    }, &mut heap).unwrap();
    let heap = heap.unwrap();
    let mut source = None;
    device.CreatePlacedResource(&heap, 0, &source_desc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut source).unwrap();
    let source = source.unwrap();
    let mut destination = None;
    device.CreatePlacedResource(&heap, destination_offset, &destination_desc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut destination).unwrap();
    let destination = destination.unwrap();
    let props = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() };
    let mut through: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&props, D3D12_HEAP_FLAG_NONE, &buffer_desc(source_bytes), D3D12_RESOURCE_STATE_COMMON, None, &mut through).unwrap();
    let through = through.unwrap();
    let texture_location = |resource: &ID3D12Resource| D3D12_TEXTURE_COPY_LOCATION {
        pResource: std::mem::transmute_copy(resource),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
    };

    // Populate the source texture once, then leave it in COPY_SOURCE.
    allocator.Reset().unwrap();
    list.Reset(allocator, None).unwrap();
    let from = D3D12_TEXTURE_COPY_LOCATION {
        pResource: std::mem::transmute_copy(upload),
        Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: source_footprint },
    };
    list.CopyTextureRegion(&texture_location(&source), 0, 0, 0, &from, None);
    list.ResourceBarrier(&[D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
            pResource: std::mem::transmute_copy(&source), Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            StateBefore: D3D12_RESOURCE_STATE_COPY_DEST, StateAfter: D3D12_RESOURCE_STATE_COPY_SOURCE,
        }) },
    }]);
    list.Close().unwrap();
    gpu.run(list).unwrap();

    for iteration in 0..iterations {
        allocator.Reset().unwrap();
        list.Reset(allocator, None).unwrap();
        for _ in 0..copies_per_list {
            list.ResourceBarrier(&[D3D12_RESOURCE_BARRIER {
                Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: std::mem::transmute_copy(&through), Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_COMMON, StateAfter: D3D12_RESOURCE_STATE_COPY_DEST,
                }) },
            }]);
            list.CopyTextureRegion(&D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&through),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: source_footprint },
            }, 0, 0, 0, &texture_location(&source), None);
            list.ResourceBarrier(&[D3D12_RESOURCE_BARRIER {
                Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: std::mem::transmute_copy(&through), Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_COPY_DEST, StateAfter: D3D12_RESOURCE_STATE_COPY_SOURCE,
                }) },
            }]);
            list.CopyTextureRegion(&texture_location(&destination), 0, 0, 0, &D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&through),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: destination_footprint },
            }, None);
            list.ResourceBarrier(&[D3D12_RESOURCE_BARRIER {
                Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: std::mem::transmute_copy(&through), Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_COPY_SOURCE, StateAfter: D3D12_RESOURCE_STATE_COMMON,
                }) },
            }]);
        }
        list.Close().unwrap();
        if let Err(why) = gpu.run(list) {
            println!("alias: HANG at list {iteration}: {why}");
            std::process::exit(2);
        }
        if iteration % 200 == 0 { eprintln!("alias: {iteration} lists done"); }
    }
    println!("alias: OK, {iterations} lists of {copies_per_list} texture-buffer-texture copies");
}

fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_else(|| "placed4".into());
    let iterations: u32 = std::env::args().nth(2).and_then(|n| n.parse().ok()).unwrap_or(2000);
    let copies_per_list: u32 = std::env::var("SYNTH_COPIES_PER_LIST")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(COPIES_PER_LIST);
    let (width, height, format) = std::env::var("SYNTH_TEXTURE")
        .ok()
        .and_then(|s| {
            let (size, format) = s.split_once(':')?;
            let (width, height) = size.split_once('x')?;
            Some((width.parse().ok()?, height.parse().ok()?, DXGI_FORMAT(format.parse().ok()?)))
        })
        .unwrap_or((16, 4, DXGI_FORMAT_R16G16B16A16_FLOAT));
    // SAFETY: plain D3D12 calls on objects made here.
    unsafe {
        if std::env::var_os("SYNTH_DEBUG").is_some() {
            let mut layer: Option<ID3D12Debug> = None;
            D3D12GetDebugInterface(&mut layer).unwrap();
            layer.unwrap().EnableDebugLayer();
        }
        let (device, name) = adapter_device();
        eprintln!("using {name}, scenario {scenario}, {iterations} lists of {copies_per_list} copies ({width}x{height} f{})", format.0);
        let queue: ID3D12CommandQueue = device
            .CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() })
            .unwrap();
        let mut gpu = Gpu {
            fence: device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap(),
            event: CreateEventW(None, false, false, None).unwrap(),
            value: 0,
            device: device.clone(),
            queue,
        };
        let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list: ID3D12GraphicsCommandList =
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).unwrap();
        list.Close().unwrap();

        // An upload block of 64 MiB the copies read from.
        let upload_props = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_UPLOAD, ..Default::default() };
        let mut upload: Option<ID3D12Resource> = None;
        device
            .CreateCommittedResource(
                &upload_props,
                D3D12_HEAP_FLAG_NONE,
                &buffer_desc(BLOCK),
                D3D12_RESOURCE_STATE_GENERIC_READ,
                None,
                &mut upload,
            )
            .unwrap();
        let upload = upload.unwrap();
        let mut data = std::ptr::null_mut();
        upload.Map(0, None, Some(&mut data)).unwrap();
        std::ptr::write_bytes(data.cast::<u8>(), 0x3c, BLOCK as usize);

        // Where the copies go: a heap of slots sized for the selected texture.
        let small = scenario == "placed4" || scenario == "churn";
        let small_alignment = if small { u64::from(D3D12_SMALL_RESOURCE_PLACEMENT_ALIGNMENT) } else { 0 };
        let mut allocation_probe = texture_desc(width, height, format, small_alignment);
        let mut allocation = device.GetResourceAllocationInfo(0, &[allocation_probe]);
        if allocation.SizeInBytes == u64::MAX || (small && allocation.Alignment != small_alignment) {
            allocation_probe.Alignment = 0;
            allocation = device.GetResourceAllocationInfo(0, &[allocation_probe]);
        }
        let slot_alignment = if small && allocation.Alignment == small_alignment { small_alignment } else { 65536 };
        let slot = allocation.SizeInBytes.next_multiple_of(slot_alignment);
        let slots = if slot > 65536 { u64::from(copies_per_list.max(64)).next_power_of_two() } else { 4096 };
        let heap_desc = D3D12_HEAP_DESC {
            SizeInBytes: slot * slots,
            Properties: D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() },
            Alignment: 65536,
            Flags: D3D12_HEAP_FLAG_ALLOW_ONLY_NON_RT_DS_TEXTURES,
        };
        let mut heap: Option<ID3D12Heap> = None;
        device.CreateHeap(&heap_desc, &mut heap).unwrap();
        let heap = heap.unwrap();
        let aliasing = std::env::var_os("SYNTH_NO_ALIASING").is_none();

        let default_props = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() };
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let (mut rows, mut row_bytes, mut total) = (0u32, 0u64, 0u64);
        let probe = texture_desc(width, height, format, 0);
        device.GetCopyableFootprints(
            &probe,
            0,
            1,
            0,
            Some(&mut footprint),
            Some(&mut rows),
            Some(&mut row_bytes),
            Some(&mut total),
        );
        eprintln!("footprint: pitch {} rows {rows} total {total}", footprint.Footprint.RowPitch);

        if scenario == "persist" {
            persist(&mut gpu, &device, &allocator, &list, &upload, &heap, slot, footprint, iterations);
            return;
        }
        if scenario == "alias" {
            alias_copy(&mut gpu, &device, &allocator, &list, &upload, width, height, format, DXGI_FORMAT(67), iterations, copies_per_list);
            return;
        }

        let mut seed = 0x9e3779b97f4a7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut live: Vec<ID3D12Resource> = Vec::new();
        for iteration in 0..iterations {
            allocator.Reset().unwrap();
            list.Reset(&allocator, None).unwrap();
            live.clear();
            for _copy in 0..copies_per_list {
                let slot_index = (next() % slots).min(slots - 1);
                let texture: ID3D12Resource = if scenario == "committed" {
                    let mut t: Option<ID3D12Resource> = None;
                    device
                        .CreateCommittedResource(
                            &default_props,
                            D3D12_HEAP_FLAG_NONE,
                            &texture_desc(width, height, format, 0),
                            D3D12_RESOURCE_STATE_COPY_DEST,
                            None,
                            &mut t,
                        )
                        .unwrap();
                    t.unwrap()
                } else {
                    // Slots are reused by textures of other shapes (churn) or the same one.
                    let (w, h, f) = match (scenario.as_str(), next() % 4) {
                        ("churn", 1) => (32, 32, DXGI_FORMAT_R8G8B8A8_UNORM),
                        ("churn", 2) => (8, 8, DXGI_FORMAT_R32_FLOAT),
                        ("churn", 3) => (64, 16, DXGI_FORMAT_R16_FLOAT),
                        _ => (width, height, format),
                    };
                    let mut desc = texture_desc(w, h, f, small_alignment);
                    let info = device.GetResourceAllocationInfo(0, &[desc]);
                    if info.SizeInBytes == u64::MAX || (small && info.Alignment != small_alignment) {
                        desc.Alignment = 0;
                    }
                    let mut t: Option<ID3D12Resource> = None;
                    device
                        .CreatePlacedResource(&heap, slot_index * slot, &desc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut t)
                        .unwrap();
                    let t = t.unwrap();
                    if aliasing {
                        let barrier = D3D12_RESOURCE_BARRIER {
                            Type: D3D12_RESOURCE_BARRIER_TYPE_ALIASING,
                            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                                Aliasing: ManuallyDrop::new(D3D12_RESOURCE_ALIASING_BARRIER {
                                    pResourceBefore: ManuallyDrop::new(None),
                                    pResourceAfter: std::mem::transmute_copy(&t),
                                }),
                            },
                        };
                        list.ResourceBarrier(&[barrier]);
                    }
                    t
                };
                // The copy of the game's upload: 896 bytes from a multiple of 512.
                let at = (next() % ((BLOCK - total) / 512 + 1)) * 512;
                let to = D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(&texture),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
                };
                let from = D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(&upload),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT { Offset: at, ..footprint },
                    },
                };
                let shaped = texture.GetDesc();
                if shaped.Width == width && shaped.Height == height && shaped.Format == format {
                    list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
                }
                let transition = D3D12_RESOURCE_BARRIER {
                    Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                    Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                    Anonymous: D3D12_RESOURCE_BARRIER_0 {
                        Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                            pResource: std::mem::transmute_copy(&texture),
                            Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                            StateBefore: D3D12_RESOURCE_STATE_COPY_DEST,
                            StateAfter: D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                        }),
                    },
                };
                list.ResourceBarrier(&[transition]);
                live.push(texture);
            }
            list.Close().unwrap();
            if let Err(why) = gpu.run(&list) {
                println!("{scenario}: HANG at list {iteration}: {why}");
                std::process::exit(2);
            }
            if iteration % 200 == 0 {
                eprintln!("{scenario}: {iteration} lists done");
            }
        }
        println!("{scenario}: OK, {iterations} lists of {copies_per_list} copies");
    }
}
