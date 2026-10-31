// SPDX-License-Identifier: MPL-2.0
//! Runs the SPIR-V that rust-gpu built on every captured call of a routine and compares bits with the
//! reference. Usage: spirv_replay <spv> <snl1|sds4|sin4|spr4|srce> <capture>. It only accepts a CPU Vulkan implementation (lavapipe), so it never touches a GPU:
//! `AGENTS.md`, "Running on the GPU". A diagnostic of ADR-0002, never a test.

use ash::vk;
use std::ffi::CStr;
use ww3_host::capture::{
    Sds4Capture, Sin4Call, Sin4Capture, Snl1Capture, Spr4Call, Spr4Capture, SrceCall, SrceCapture,
};
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
    let p = c.params(st.params, sds.params, sin.params(), spr.params);
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
    let out = dispatch(code, c"srce", &slots, points);
    let ours = floats(&out[27]);
    compare("SPEC", &out[26], &rs) & compare("scalars", bytes(&ours), &ro) & compare("WHITECAP", &out[WHITECAP], &rw)
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
        "srce" => srce_run(&code, &path),
        other => panic!("unknown kernel {other}"),
    };
    println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!ok as i32);
}
