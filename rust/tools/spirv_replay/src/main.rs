// SPDX-License-Identifier: MPL-2.0
//! Runs the SPIR-V that rust-gpu built on every captured call of a routine and compares bits with the
//! reference. Usage: spirv_replay <spv> <snl1|sds4|sin4|spr4|spr3|spr1|srce|sbt4|sic1|sis1|sds3|flx1|sin1|sds1|sds6> <capture>, or <spv> libm <inputs>. It only accepts a CPU Vulkan implementation (lavapipe), so it never touches a GPU:
//! `AGENTS.md`, "Running on the GPU". A diagnostic of ADR-0002, never a test.

use ash::vk;
use std::ffi::CStr;
use ww3_host::capture::{
    Flx1Capture, Sbt4Call, Sbt4Capture, Sds1Call, Sds1Capture, Sds4Capture, Sic1Call, Sic1Capture, Sin1Capture, Sin4Call, Sin4Capture, Snl1Capture, Spr1Capture, Spr4Call, Spr4Capture, SrceCall, SrceCapture,
};
use ww3_host::capture::{Sin3Call, Sin3Capture};
use ww3_kernels::srce::{SrceOut, work_lens, work_small};
use ww3_kernels::{sds4, snl1};

struct Buffer {
    buffer: vk::Buffer,
    size: u64,
    map: *mut u8,
}

fn bytes<T>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), size_of_val(v)) }
}

/// One storage buffer, by binding: data to upload, or an empty output of this many bytes.
enum Slot<'a> {
    In(&'a [u8]),
    Out(usize),
}

/// Dispatches `entry_name` over `points` and returns the final contents of every buffer.
fn dispatch(code: &[u8], entry_name: &CStr, slots: &[Slot], points: usize) -> Vec<Vec<u8>> {
    let words: Vec<u32> =
        code.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();

    unsafe {
        let entry = ash::Entry::load().expect("Vulkan loader");
        let app = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_2);
        let instance = entry
            .create_instance(&vk::InstanceCreateInfo::default().application_info(&app), None)
            .unwrap();
        let phys = instance
            .enumerate_physical_devices()
            .unwrap()
            .into_iter()
            .find(|d| {
                instance.get_physical_device_properties(*d).device_type
                    == vk::PhysicalDeviceType::CPU
            })
            .expect("no CPU Vulkan implementation: this tool never runs on a GPU");
        let props = instance.get_physical_device_properties(phys);
        println!("device   {}", CStr::from_ptr(props.device_name.as_ptr()).to_string_lossy());

        let family = instance
            .get_physical_device_queue_family_properties(phys)
            .iter()
            .position(|q| q.queue_flags.contains(vk::QueueFlags::COMPUTE))
            .unwrap() as u32;
        let features = vk::PhysicalDeviceFeatures::default().shader_float64(true).shader_int64(true);
        let priorities = [1.0];
        let queues =
            [vk::DeviceQueueCreateInfo::default().queue_family_index(family).queue_priorities(&priorities)];
        let device = instance
            .create_device(
                phys,
                &vk::DeviceCreateInfo::default().queue_create_infos(&queues).enabled_features(&features),
                None,
            )
            .unwrap();
        let memory = instance.get_physical_device_memory_properties(phys);
        let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;

        let make = |size: usize| -> Buffer {
            let info = vk::BufferCreateInfo::default()
                .size(size as u64)
                .usage(vk::BufferUsageFlags::STORAGE_BUFFER);
            let buffer = device.create_buffer(&info, None).unwrap();
            let req = device.get_buffer_memory_requirements(buffer);
            let index = (0..memory.memory_type_count)
                .find(|i| {
                    req.memory_type_bits & (1 << i) != 0
                        && memory.memory_types[*i as usize].property_flags.contains(wanted)
                })
                .unwrap();
            let alloc = vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(index);
            let mem = device.allocate_memory(&alloc, None).unwrap();
            device.bind_buffer_memory(buffer, mem, 0).unwrap();
            let map = device.map_memory(mem, 0, req.size, vk::MemoryMapFlags::empty()).unwrap().cast();
            Buffer { buffer, size: size as u64, map }
        };
        let input = |data: &[u8]| -> Buffer {
            let b = make(data.len());
            std::ptr::copy_nonoverlapping(data.as_ptr(), b.map, data.len());
            b
        };

        let buffers: Vec<Buffer> = slots
            .iter()
            .map(|slot| match slot {
                Slot::In(data) => input(data),
                Slot::Out(size) => make(*size),
            })
            .collect();

        let bindings: Vec<_> = (0..buffers.len() as u32)
            .map(|i| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(i)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
            })
            .collect();
        let layout = device
            .create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
            .unwrap();
        let layouts = [layout];
        let pipeline_layout = device
            .create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts), None)
            .unwrap();
        let module =
            device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None).unwrap();
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(module)
            .name(entry_name);
        let pipeline = device
            .create_compute_pipelines(
                vk::PipelineCache::null(),
                &[vk::ComputePipelineCreateInfo::default().stage(stage).layout(pipeline_layout)],
                None,
            )
            .unwrap()[0];

        let sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: buffers.len() as u32,
        }];
        let pool = device
            .create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes),
                None,
            )
            .unwrap();
        let set = device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts),
            )
            .unwrap()[0];
        let infos: Vec<[vk::DescriptorBufferInfo; 1]> = buffers
            .iter()
            .map(|b| [vk::DescriptorBufferInfo { buffer: b.buffer, offset: 0, range: b.size }])
            .collect();
        let writes: Vec<_> = infos
            .iter()
            .enumerate()
            .map(|(i, info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(i as u32)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(info)
            })
            .collect();
        device.update_descriptor_sets(&writes, &[]);

        let cmd_pool = device
            .create_command_pool(&vk::CommandPoolCreateInfo::default().queue_family_index(family), None)
            .unwrap();
        let cmd = device
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default().command_pool(cmd_pool).command_buffer_count(1),
            )
            .unwrap()[0];
        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default()).unwrap();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::COMPUTE, pipeline_layout, 0, &[set], &[]);
        device.cmd_dispatch(cmd, points.div_ceil(64) as u32, 1, 1);
        device.end_command_buffer(cmd).unwrap();
        let fence = device.create_fence(&vk::FenceCreateInfo::default(), None).unwrap();
        let cmds = [cmd];
        let queue = device.get_device_queue(family, 0);
        let start = std::time::Instant::now();
        device.queue_submit(queue, &[vk::SubmitInfo::default().command_buffers(&cmds)], fence).unwrap();
        device.wait_for_fences(&[fence], true, u64::MAX).unwrap();
        println!("dispatch {points} points in {:?}", start.elapsed());

        buffers
            .iter()
            .map(|b| std::slice::from_raw_parts(b.map, b.size as usize).to_vec())
            .collect()
    }
}

fn floats(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()
}

/// Compares one output buffer. Returns true when every bit is equal.
fn compare(name: &str, ours: &[u8], theirs: &[f32]) -> bool {
    let ours = floats(ours);
    assert_eq!(ours.len(), theirs.len());
    let bad: Vec<(f32, f32)> = ours
        .iter()
        .zip(theirs.iter())
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .map(|(x, y)| (*x, *y))
        .collect();
    let ulp = bad.iter().map(|(x, y)| (x.to_bits() as i64 - y.to_bits() as i64).unsigned_abs()).max();
    println!("{name:9}{:>9} values, {} differ, max {} ULP", ours.len(), bad.len(), ulp.unwrap_or(0));
    // ULP says little where a term nearly cancels, so also the error against the array's scale.
    let scale = theirs.iter().fold(0_f32, |m, x| m.max(x.abs()));
    let (mut rel, mut abs) = (0_f64, 0_f64);
    for (x, y) in &bad {
        let d = (*x as f64 - *y as f64).abs();
        abs = abs.max(d);
        rel = rel.max(d / (y.abs() as f64).max(1e-300));
    }
    println!("         max |diff| {abs:.3e}, {:.3e} of the array's peak; max relative {rel:.3e}", abs / scale as f64);
    if let Some((x, y)) = bad.first() {
        println!("         first: ours {x:e}, reference {y:e}");
    }
    bad.is_empty()
}

fn snl1_run(code: &[u8], path: &str) -> bool {
    let c = Snl1Capture::read(path);
    let t = c.rebuild();
    let p = t.params;
    let nspec = p.nspec as usize;
    let points = c.calls.len();
    let concat = |f: fn(&ww3_host::capture::Snl1Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, cg, s_ref, d_ref) = (concat(|k| &k.a), concat(|k| &k.cg), concat(|k| &k.s), concat(|k| &k.d));
    let kd: Vec<f32> = c.calls.iter().map(|k| k.kdmean).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&c.sig)),
        Slot::In(bytes(&t.af11)),
        Slot::In(bytes(&t.pm)),
        Slot::In(bytes(&t.ic)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&kd)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * snl1::work_len(&p) * 4),
    ];
    let out = dispatch(code, c"snl1", &slots, points);
    compare("S", &out[8], &s_ref) & compare("D", &out[9], &d_ref)
}

fn sds4_run(code: &[u8], path: &str) -> bool {
    let c = Sds4Capture::read(path);
    let p = c.params;
    let nspec = p.nspec as usize;
    let points = c.calls.len();
    let (lw, li) = sds4::work_len(&p);
    let concat = |f: fn(&ww3_host::capture::Sds4Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, k, cg, wc) = (concat(|x| &x.a), concat(|x| &x.k), concat(|x| &x.cg), concat(|x| &x.whitecap_in));
    let pts: Vec<_> = c.calls.iter().map(|x| x.point).collect();
    let (rs, rd, rb, rw) =
        (concat(|x| &x.srhs), concat(|x| &x.ddiag), concat(|x| &x.brlambda), concat(|x| &x.whitecap));
    // An empty table still needs a buffer to bind.
    let one = [0u8; 4];
    let table = |b: &'_ [u8]| -> Vec<u8> { if b.is_empty() { one.to_vec() } else { b.to_vec() } };
    let owned = [
        table(bytes(&c.cumulw)),
        table(bytes(&c.iktab)),
        table(bytes(&c.dcki)),
        table(bytes(&c.qbi)),
    ];
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&c.sig)),
        Slot::In(bytes(&c.dden)),
        Slot::In(bytes(&c.ec2)),
        Slot::In(bytes(&c.es2)),
        Slot::In(bytes(&c.esc)),
        Slot::In(bytes(&c.ecos)),
        Slot::In(bytes(&c.esin)),
        Slot::In(bytes(&c.satindices)),
        Slot::In(bytes(&c.satweights)),
        Slot::In(&owned[0]),
        Slot::In(&owned[1]),
        Slot::In(&owned[2]),
        Slot::In(&owned[3]),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&k)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::In(bytes(&wc)),
        Slot::Out(points * lw * 4),
        Slot::Out(points * li * 4),
    ];
    let out = dispatch(code, c"sds4", &slots, points);
    compare("SRHS", &out[18], &rs)
        & compare("DDIAG", &out[19], &rd)
        & compare("BRLAMBDA", &out[20], &rb)
        & compare("WHITECAP", &out[21], &rw)
}

fn sin4_run(code: &[u8], path: &str) -> bool {
    let c = Sin4Capture::read(path);
    let p = c.params();
    let nspec = p.nspec as usize;
    let points = c.calls.len();
    let concat = |f: fn(&Sin4Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, cg, k) = (concat(|x| &x.a), concat(|x| &x.cg), concat(|x| &x.k));
    let pts: Vec<_> = c.calls.iter().map(|x| x.point).collect();
    let (rs, rd) = (concat(|x| &x.s), concat(|x| &x.d));
    let rl: Vec<u32> = c.calls.iter().flat_map(|x| x.llws.iter().copied()).collect();
    let ro: Vec<f32> =
        c.calls.iter().flat_map(|x| [x.out.tauwx, x.out.tauwy, x.out.tauwnx, x.out.tauwny]).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&c.sig)),
        Slot::In(bytes(&c.sig2)),
        Slot::In(bytes(&c.dden)),
        Slot::In(bytes(&c.dden2)),
        Slot::In(bytes(&c.ecos)),
        Slot::In(bytes(&c.esin)),
        Slot::In(bytes(&c.fwtable)),
        Slot::In(bytes(&c.tauhft)),
        Slot::In(bytes(&c.tauhft2)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&k)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * 16),
    ];
    let out = dispatch(code, c"sin4", &slots, points);
    let llws = out[16].chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let bad = llws.zip(rl.iter()).filter(|(a, b)| a != *b).count();
    println!("LLWS     {bad} differ");
    compare("S", &out[14], &rs) & compare("D", &out[15], &rd) & compare("TAUW", &out[17], &ro) & (bad == 0)
}

/// `W3SIN3` on every captured call, with the module tables as the reference left them.
fn sin3_run(code: &[u8], path: &str) -> bool {
    use ww3_kernels::sin3::Sin3Out;
    let c = Sin3Capture::read(path);
    let p = c.params;
    let nspec = p.nspec as usize;
    let points = c.calls.len();
    let concat = |f: fn(&Sin3Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, cg, k) = (concat(|x| &x.a), concat(|x| &x.cg), concat(|x| &x.k));
    let pts: Vec<_> = c.calls.iter().map(|x| x.point).collect();
    let (rs, rd) = (concat(|x| &x.s), concat(|x| &x.d));
    let rl: Vec<u32> = c.calls.iter().flat_map(|x| x.llws.iter().copied()).collect();
    let ro: Vec<f32> =
        c.calls.iter().flat_map(|x| [x.out.tauwx, x.out.tauwy, x.out.tauwnx, x.out.tauwny]).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&c.sig)),
        Slot::In(bytes(&c.sig2)),
        Slot::In(bytes(&c.dden2)),
        Slot::In(bytes(&c.ecos)),
        Slot::In(bytes(&c.esin)),
        Slot::In(bytes(&c.tauhft)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&k)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * size_of::<Sin3Out>()),
    ];
    let out = dispatch(code, c"sin3", &slots, points);
    let llws = out[13].chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let bad = llws.zip(rl.iter()).filter(|(a, b)| a != *b).count();
    println!("LLWS     {bad} differ");
    // The four stresses out of the Sin3Out records; `reach` follows them.
    let tau: Vec<u8> = out[14].chunks_exact(size_of::<Sin3Out>()).flat_map(|r| r[..16].iter().copied()).collect();
    compare("S", &out[11], &rs) & compare("D", &out[12], &rd) & compare("TAUW", &tau, &ro) & (bad == 0)
}

fn spr4_run(code: &[u8], path: &str) -> bool {
    let c = Spr4Capture::read(path);
    let p = c.params;
    let points = c.calls.len();
    let concat = |f: fn(&Spr4Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, cg, wn, ro) = (concat(|x| &x.a), concat(|x| &x.cg), concat(|x| &x.wn), concat(|x| &x.out));
    let llws: Vec<u32> = c.calls.iter().flat_map(|x| x.llws.iter().copied()).collect();
    let pts: Vec<_> = c.calls.iter().map(|x| x.point).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&c.sig)),
        Slot::In(bytes(&c.dden)),
        Slot::In(bytes(&c.ecos)),
        Slot::In(bytes(&c.esin)),
        Slot::In(bytes(&c.taut)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&wn)),
        Slot::In(bytes(&llws)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * 2 * p.nk as usize * 4),
        Slot::Out(points * 48),
    ];
    let out = dispatch(code, c"spr4", &slots, points);
    compare("W3SPR4", &out[12], &ro)
}

/// `W3SPR3` on every captured call (ST3 build).
fn spr3_run(code: &[u8], path: &str) -> bool {
    use ww3_host::capture::{Spr3Call, Spr3Capture};
    let c = Spr3Capture::read(path);
    let p = c.params;
    let points = c.calls.len();
    let concat = |f: fn(&Spr3Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, cg, wn, ro) = (concat(|x| &x.a), concat(|x| &x.cg), concat(|x| &x.wn), concat(|x| &x.out));
    let llws: Vec<u32> = c.calls.iter().flat_map(|x| x.llws.iter().copied()).collect();
    let pts: Vec<_> = c.calls.iter().map(|x| x.point).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&c.sig)),
        Slot::In(bytes(&c.dden)),
        Slot::In(bytes(&c.taut)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&wn)),
        Slot::In(bytes(&llws)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * 2 * p.nk as usize * 4),
        Slot::Out(points * size_of::<ww3_kernels::spr3::Spr3Out>()),
    ];
    let out = dispatch(code, c"spr3", &slots, points);
    compare("W3SPR3", &out[10], &ro)
}

fn spr1_run(code: &[u8], path: &str) -> bool {
    let c = Spr1Capture::read(path);
    let points = c.calls.len();
    let cat = |f: fn(&ww3_host::capture::Spr1Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, cg, wn) = (cat(|x| &x.a), cat(|x| &x.cg), cat(|x| &x.wn));
    let ro: Vec<f32> = c.calls.iter().flat_map(|x| [x.out.emean, x.out.fmean, x.out.wnmean, x.out.amax]).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&c.params))),
        Slot::In(bytes(&c.sig)),
        Slot::In(bytes(&c.dden)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&wn)),
        Slot::Out(points * c.params.nk as usize * 4),
        Slot::Out(points * 16),
    ];
    let out = dispatch(code, c"spr1", &slots, points);
    compare("W3SPR1", &out[7], &ro)
}

/// `path` is `<dir>/srce_<tag>.bin`: the tables come from the same case's other captures.
fn srce_run(code: &[u8], path: &str) -> bool {
    let sib = |r: &str| path.replace("/srce_", &format!("/{r}_"));
    let c = SrceCapture::read(path);
    let (snl, sds, sin, spr) = (
        Snl1Capture::read(&sib("snl1")),
        Sds4Capture::read(&sib("sds4")),
        Sin4Capture::read(&sib("sin4")),
        Spr4Capture::read(&sib("spr4")),
    );
    let st = snl.rebuild();
    let mut p = c.params(st.params, sds.params, sin.params(), spr.params);
    // A capture from a BT4 build: W3SRCE calls W3SBT4, ERFTABLE built on the host.
    let mut erf = vec![0.0f32; ww3_kernels::sbt4::SIZEERFTABLE as usize + 1];
    ww3_kernels::sbt4::tabu_erf(&mut erf);
    if p.btopt == ww3_kernels::srce::SRCE_BT4 {
        p.sbt4 = Sbt4Capture::read(&sib("sbt4")).params;
        p.sbt4.delxerf = ww3_kernels::sbt4::delxerf();
        println!("BT4: W3SRCE calls W3SBT4");
    }
    let nspec = p.nspec as usize;
    let points = c.calls.len();
    let concat = |f: fn(&SrceCall) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (spec, wn1, cg1) = (concat(|x| &x.spec), concat(|x| &x.wn1), concat(|x| &x.cg1));
    let (rs, ro, rw) = (concat(|x| &x.spec_out), concat(|x| &x.out), concat(|x| &x.whitecap));
    let pts: Vec<_> = c.calls.iter().map(|x| x.point).collect();
    let (l_snl, l_sds, l_ids) = work_lens(&p);
    let one = [0u8; 4];
    let tab = |b: &[u8]| -> Vec<u8> { if b.is_empty() { one.to_vec() } else { b.to_vec() } };
    let owned = [tab(bytes(&sds.cumulw)), tab(bytes(&sds.iktab)), tab(bytes(&sds.dcki)), tab(bytes(&sds.qbi))];
    let mut slots = vec![
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&sin.sig)),
        Slot::In(bytes(&sin.sig2)),
        Slot::In(bytes(&sin.dden)),
        Slot::In(bytes(&sin.dden2)),
        Slot::In(bytes(&sds.ec2)),
        Slot::In(bytes(&sds.es2)),
        Slot::In(bytes(&sds.esc)),
        Slot::In(bytes(&sin.ecos)),
        Slot::In(bytes(&sin.esin)),
        Slot::In(bytes(&st.af11)),
        Slot::In(bytes(&st.pm)),
        Slot::In(bytes(&st.ic)),
        Slot::In(bytes(&sds.satindices)),
        Slot::In(bytes(&sds.satweights)),
        Slot::In(&owned[0]),
        Slot::In(&owned[1]),
        Slot::In(&owned[2]),
        Slot::In(&owned[3]),
        Slot::In(bytes(&sin.fwtable)),
        Slot::In(bytes(&sin.tauhft)),
        Slot::In(bytes(&sin.tauhft2)),
        Slot::In(bytes(&sin.taut)),
        Slot::In(bytes(&pts)),
        Slot::In(bytes(&wn1)),
        Slot::In(bytes(&cg1)),
        Slot::In(bytes(&spec)),
        Slot::Out(points * size_of::<SrceOut>()),
    ];
    // After SPEC, out: 18 spectrum-sized arrays, w_snl, w_sds, iw_sds, w_small.
    const WHITECAP: usize = 28 + 22;
    for _ in 0..18 {
        slots.push(Slot::Out(points * nspec * 4));
    }
    slots.push(Slot::Out(points * l_snl * 4));
    slots.push(Slot::Out(points * l_sds * 4));
    slots.push(Slot::Out(points * l_ids * 4));
    slots.push(Slot::Out(points * work_small(&p) * 4));
    slots.push(Slot::Out(points * 16));
    slots.push(Slot::Out(points * 16));
    slots.push(Slot::Out(points * 48));
    // Binding 53, `active`: the time loop marks the points due a call; every captured call is one.
    let active = vec![1u32; points];
    slots.push(Slot::In(bytes(&active)));
    // Bindings 54 and 55: ERFTABLE, and BEDFORM in and out, three per point.
    let (bin, rb): (Vec<f32>, Vec<f32>) =
        (c.calls.iter().flat_map(|k| k.bedform_in).collect(), c.calls.iter().flat_map(|k| k.bedform).collect());
    slots.push(Slot::In(bytes(&erf)));
    slots.push(Slot::In(bytes(&bin)));
    let out = dispatch(code, c"srce", &slots, points);
    let ours = floats(&out[27]);
    compare("SPEC", &out[26], &rs)
        & compare("scalars", bytes(&ours), &ro)
        & compare("WHITECAP", &out[WHITECAP], &rw)
        & compare("BEDFORM", &out[55], &rb)
}

/// `W3SBT4` on every captured call, with the `ERFTABLE` that `TABU_ERF` builds on the host.
fn sbt4_run(code: &[u8], path: &str) -> bool {
    use ww3_kernels::sbt4::{SIZEERFTABLE, Sbt4Out, delxerf, tabu_erf};
    let c = Sbt4Capture::read(path);
    let mut p = c.params;
    p.delxerf = delxerf();
    let mut erf = vec![0.0f32; SIZEERFTABLE as usize + 1];
    tabu_erf(&mut erf);
    let (nk, nspec) = (p.nk as usize, (p.nk * p.nth) as usize);
    let points = c.calls.len();
    let cat = |f: fn(&Sbt4Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|k| f(k).iter().copied()).collect() };
    let (a, cg, wn, bin) = (cat(|k| &k.a), cat(|k| &k.cg), cat(|k| &k.wn), cat(|k| &k.bedform_in));
    let (rs, rd, rb) = (cat(|k| &k.s), cat(|k| &k.d), cat(|k| &k.bedform));
    let rt: Vec<f32> = c.calls.iter().flat_map(|k| k.taubbl).collect();
    let pts: Vec<_> = c.calls.iter().map(|k| k.point).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&c.sig)),
        Slot::In(bytes(&c.dden)),
        Slot::In(bytes(&c.ecos)),
        Slot::In(bytes(&c.esin)),
        Slot::In(bytes(&c.fwtable)),
        Slot::In(bytes(&erf)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&wn)),
        Slot::In(bytes(&pts)),
        Slot::In(bytes(&bin)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nk * 4),
        Slot::Out(points * size_of::<Sbt4Out>()),
    ];
    let out = dispatch(code, c"sbt4", &slots, points);
    // TAUBBL out of the Sbt4Out records: two floats, then the branch.
    let o = &out[15];
    let tau: Vec<u8> = o.chunks_exact(size_of::<Sbt4Out>()).flat_map(|r| r[..8].iter().copied()).collect();
    compare("S", &out[12], &rs) & compare("D", &out[13], &rd) & compare("BEDFORM", &out[11], &rb) & compare("TAUBBL", &tau, &rt)
}

/// `W3SIC1` on every captured call.
fn sic1_run(code: &[u8], path: &str) -> bool {
    use ww3_kernels::sic1::Sic1Out;
    let c = Sic1Capture::read(path);
    let ns = (c.params.nk * c.params.nth) as usize;
    let points = c.calls.len();
    let cat = |f: fn(&Sic1Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|k| f(k).iter().copied()).collect() };
    let (a, cg, rs, rd) = (cat(|k| &k.a), cat(|k| &k.cg), cat(|k| &k.s), cat(|k| &k.d));
    let ice: Vec<f32> = c.calls.iter().map(|k| k.icecoef1).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&c.params))),
        Slot::In(bytes(&c.mapwn)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&ice)),
        Slot::Out(points * ns * 4),
        Slot::Out(points * ns * 4),
        Slot::Out(points * size_of::<Sic1Out>()),
    ];
    let out = dispatch(code, c"sic1", &slots, points);
    compare("S", &out[5], &rs) & compare("D", &out[6], &rd)
}

/// `W3SIS1` on every captured call.
fn sis1_run(code: &[u8], path: &str) -> bool {
    use ww3_host::capture::Sis1Capture;
    let c = Sis1Capture::read(path);
    let p = c.params;
    let nspec = (p.nk * p.nth) as usize;
    let points = c.calls.len();
    let a: Vec<f32> = c.calls.iter().flat_map(|k| k.a.iter().copied()).collect();
    let rs: Vec<f32> = c.calls.iter().flat_map(|k| k.s.iter().copied()).collect();
    let pts: Vec<_> = c.calls.iter().map(|k| k.point).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * nspec * 4),
    ];
    let out = dispatch(code, c"sis1", &slots, points);
    compare("S", &out[3], &rs)
}

/// `W3SDS3` on every captured call.
fn sds3_run(code: &[u8], path: &str) -> bool {
    use ww3_host::capture::{Sds3Call, Sds3Capture};
    let c = Sds3Capture::read(path);
    let p = c.params;
    let nspec = (p.nk * p.nth) as usize;
    let points = c.calls.len();
    let cat = |f: fn(&Sds3Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|k| f(k).iter().copied()).collect() };
    let (a, k, rs, rd) = (cat(|k| &k.a), cat(|k| &k.k), cat(|k| &k.s), cat(|k| &k.d));
    let pts: Vec<_> = c.calls.iter().map(|k| k.point).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&k)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
    ];
    let out = dispatch(code, c"sds3", &slots, points);
    compare("S", &out[4], &rs) & compare("D", &out[5], &rd)
}

/// `W3SDS6` on every captured call: one thread per call, with a scratch range of four `NK` each.
fn sds6_run(code: &[u8], path: &str) -> bool {
    use ww3_host::capture::{Sds6Call, Sds6Capture};
    let c = Sds6Capture::read(path);
    let p = c.params;
    let (nk, nspec) = (p.nk as usize, (p.nk * p.nth) as usize);
    let points = c.calls.len();
    let cat = |f: fn(&Sds6Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|k| f(k).iter().copied()).collect() };
    let (a, cg, wn, rs, rd) = (cat(|k| &k.a), cat(|k| &k.cg), cat(|k| &k.wn), cat(|k| &k.s), cat(|k| &k.d));
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&p))),
        Slot::In(bytes(&c.sig2)),
        Slot::In(bytes(&c.dsii)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&cg)),
        Slot::In(bytes(&wn)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * 4 * nk * 4),
    ];
    let out = dispatch(code, c"sds6", &slots, points);
    compare("S", &out[6], &rs) & compare("D", &out[7], &rd)
}

/// `W3FLX1` on every captured call: one point per call, `Flx1Out` records back.
fn flx1_run(code: &[u8], path: &str) -> bool {
    use ww3_kernels::flx1::Flx1Out;
    let c = Flx1Capture::read(path);
    let points = c.calls.len();
    let ins: Vec<_> = c.calls.iter().map(|k| k.input).collect();
    let slots = [Slot::In(bytes(&ins)), Slot::Out(points * size_of::<Flx1Out>())];
    let out = dispatch(code, c"flx1", &slots, points);
    let recs: Vec<&[u8]> = out[1].chunks_exact(size_of::<Flx1Out>()).collect();
    let mut ok = true;
    for (j, name) in ["UST", "USTD", "Z0", "CD"].into_iter().enumerate() {
        let ours: Vec<u8> = recs.iter().flat_map(|r| r[4 * j..4 * j + 4].iter().copied()).collect();
        let theirs: Vec<f32> = c.calls.iter().map(|k| k.out[j]).collect();
        ok &= compare(name, &ours, &theirs);
    }
    let err: u32 = recs.iter().map(|r| u32::from_le_bytes(r[16..20].try_into().unwrap())).sum();
    println!("ZWND != 10 {err:>7} calls");
    ok & (err == 0)
}

/// `W3SIN1` on every captured call.
fn sin1_run(code: &[u8], path: &str) -> bool {
    let c = Sin1Capture::read(path);
    let nspec = c.params.nspec as usize;
    let points = c.calls.len();
    let cat = |f: fn(&ww3_host::capture::Sin1Call) -> &[f32]| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, kk, rs, rd) = (cat(|k| &k.a), cat(|k| &k.k), cat(|k| &k.s), cat(|k| &k.d));
    let pts: Vec<_> = c.calls.iter().map(|k| k.point).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&c.params))),
        Slot::In(bytes(&c.sig2)),
        Slot::In(bytes(&c.ecos)),
        Slot::In(bytes(&c.esin)),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&kk)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
    ];
    let out = dispatch(code, c"sin1", &slots, points);
    compare("S", &out[7], &rs) & compare("D", &out[8], &rd)
}

/// `W3SDS1` on every captured call.
fn sds1_run(code: &[u8], path: &str) -> bool {
    let c = Sds1Capture::read(path);
    let nspec = (c.params.nk * c.params.nth) as usize;
    let points = c.calls.len();
    let cat = |f: fn(&Sds1Call) -> &[f32]| -> Vec<f32> { c.calls.iter().flat_map(|k| f(k).iter().copied()).collect() };
    let (a, k, rs, rd) = (cat(|k| &k.a), cat(|k| &k.k), cat(|k| &k.s), cat(|k| &k.d));
    let pts: Vec<_> = c.calls.iter().map(|k| k.point).collect();
    let slots = [
        Slot::In(bytes(std::slice::from_ref(&c.params))),
        Slot::In(bytes(&a)),
        Slot::In(bytes(&k)),
        Slot::In(bytes(&pts)),
        Slot::Out(points * nspec * 4),
        Slot::Out(points * nspec * 4),
    ];
    let out = dispatch(code, c"sds1", &slots, points);
    compare("S", &out[4], &rs) & compare("D", &out[5], &rd)
}

/// Every `ww3-libm` function in SPIR-V against the same source on this host, on random finite inputs.
/// The host build is the one `just libm-bits` proves against glibc.
fn libm_run(code: &[u8], n: &str) -> bool {
    let n: usize = n.parse().expect("number of inputs");
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 16) as u32
    };
    // Exponents near 1 are what a kernel sees, so half the inputs are there.
    let mut draw = |wide: bool| loop {
        let b = next();
        let b = if wide { b } else { (b & 0x807f_ffff) | ((b >> 24) % 40 + 107) << 23 };
        let x = f32::from_bits(b);
        if x.is_finite() {
            return x;
        }
    };
    let x: Vec<f32> = (0..n).map(|i| draw(i % 2 == 0)).collect();
    let y: Vec<f32> = (0..n).map(|i| draw(i % 2 == 0)).collect();
    let abs: Vec<f32> = x.iter().map(|v| v.abs()).collect();
    type F = fn(f32, f32) -> (f32, f32);
    let funcs: [(&str, F); 12] = [
        ("expf", |a, _| (ww3_libm::expf(a), 0.)),
        ("logf", |a, _| (ww3_libm::logf(a), 0.)),
        ("powf", |a, b| (ww3_libm::powf(a, b), 0.)),
        ("sincosf", |a, _| ww3_libm::sincosf(a)),
        ("tanhf", |a, _| (ww3_libm::tanhf(a), 0.)),
        ("atan2f", |a, b| (ww3_libm::atan2f(a, b), 0.)),
        ("log10f", |a, _| (ww3_libm::log10f(a), 0.)),
        ("sqrtf", |a, _| (ww3_libm::sqrtf(a), 0.)),
        ("atanf", |a, _| (ww3_libm::atanf(a), 0.)),
        ("expm1f", |a, _| (ww3_libm::expm1f(a), 0.)),
        ("sinhf", |a, _| (ww3_libm::sinhf(a), 0.)),
        ("erff", |a, _| (ww3_libm::erff(a), 0.)),
    ];
    let mut ok = true;
    for (i, (name, f)) in funcs.iter().enumerate() {
        // `logf`, `powf`'s base and `sqrtf` of a negative are NaN on both sides, and say nothing.
        let xs: &Vec<f32> = if matches!(i, 1 | 2 | 6 | 7) { &abs } else { &x };
        let which = [i as u32];
        let slots = [
            Slot::In(bytes(&which)),
            Slot::In(bytes(xs)),
            Slot::In(bytes(&y)),
            Slot::Out(n * 4),
            Slot::Out(n * 4),
        ];
        let out = dispatch(code, c"libm_probe", &slots, n);
        let host: Vec<(f32, f32)> = xs.iter().zip(y.iter()).map(|(a, b)| f(*a, *b)).collect();
        let (r, r2) = (floats(&out[3]), floats(&out[4]));
        let same = |a: f32, b: f32| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan());
        let mut bad = 0;
        let mut first = None;
        for k in 0..n {
            if !same(r[k], host[k].0) || (i == 3 && !same(r2[k], host[k].1)) {
                bad += 1;
                first.get_or_insert(k);
            }
        }
        print!("{name:8} {n} inputs, {bad} differ");
        if let Some(k) = first {
            print!("; first x {:08x} y {:08x}: ours {:08x} {:08x}, host {:08x} {:08x}",
                xs[k].to_bits(), y[k].to_bits(), r[k].to_bits(), r2[k].to_bits(), host[k].0.to_bits(), host[k].1.to_bits());
        }
        println!();
        ok &= bad == 0;
    }
    ok
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (spv, kernel, path) =
        (args.next().expect("spv"), args.next().expect("kernel"), args.next().expect("capture"));
    let code = std::fs::read(&spv).unwrap();
    let ok = match kernel.as_str() {
        "snl1" => snl1_run(&code, &path),
        "sds4" => sds4_run(&code, &path),
        "sin4" => sin4_run(&code, &path),
        "spr4" => spr4_run(&code, &path),
        "spr3" => spr3_run(&code, &path),
        "spr1" => spr1_run(&code, &path),
        "srce" => srce_run(&code, &path),
        "sbt4" => sbt4_run(&code, &path),
        "sin3" => sin3_run(&code, &path),
        "sic1" => sic1_run(&code, &path),
        "sis1" => sis1_run(&code, &path),
        "sds3" => sds3_run(&code, &path),
        "flx1" => flx1_run(&code, &path),
        "sin1" => sin1_run(&code, &path),
        "sds1" => sds1_run(&code, &path),
        "sds6" => sds6_run(&code, &path),
        "libm" => libm_run(&code, &path),
        other => panic!("unknown kernel {other}"),
    };
    println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!ok as i32);
}
