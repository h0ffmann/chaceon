// SPDX-License-Identifier: MPL-2.0
//! Runs the SPIR-V that rust-gpu built on every captured `W3SNL1` call and compares bits with the
//! reference. It only accepts a CPU Vulkan implementation (lavapipe), so it never touches a GPU:
//! `AGENTS.md`, "Running on the GPU". A diagnostic of ADR-0002, never a test.

use ash::vk;
use std::ffi::CStr;
use ww3_host::capture::Snl1Capture;
use ww3_kernels::snl1::work_len;

struct Buffer {
    buffer: vk::Buffer,
    size: u64,
    map: *mut u8,
}

fn bytes<T>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), size_of_val(v)) }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (spv, path) = (args.next().expect("spv"), args.next().expect("capture"));
    let c = Snl1Capture::read(&path);
    let t = c.rebuild();
    let p = t.params;
    let (nk, nspec) = (p.nk as usize, p.nspec as usize);
    let points = c.calls.len();
    let concat = |f: fn(&ww3_host::capture::Snl1Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|k| f(k).iter().copied()).collect()
    };
    let (a, cg, s_ref, d_ref) = (concat(|k| &k.a), concat(|k| &k.cg), concat(|k| &k.s), concat(|k| &k.d));
    let kd: Vec<f32> = c.calls.iter().map(|k| k.kdmean).collect();
    assert_eq!(cg.len(), points * nk);

    let code = std::fs::read(&spv).unwrap();
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

        let params = std::slice::from_ref(&p);
        let buffers = [
            input(bytes(params)),
            input(bytes(&c.sig)),
            input(bytes(&t.af11)),
            input(bytes(&t.pm)),
            input(bytes(&t.ic)),
            input(bytes(&a)),
            input(bytes(&cg)),
            input(bytes(&kd)),
            make(points * nspec * 4),
            make(points * nspec * 4),
            make(points * work_len(&p) * 4),
        ];

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
            .name(c"snl1");
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

        let read = |b: &Buffer| std::slice::from_raw_parts(b.map.cast::<f32>(), points * nspec);
        let mut ok = true;
        for (name, ours, theirs) in [("S", read(&buffers[8]), &s_ref), ("D", read(&buffers[9]), &d_ref)] {
            let bad: Vec<(f32, f32)> = ours
                .iter()
                .zip(theirs.iter())
                .filter(|(x, y)| x.to_bits() != y.to_bits())
                .map(|(x, y)| (*x, *y))
                .collect();
            let ulp = bad.iter().map(|(x, y)| (x.to_bits() as i64 - y.to_bits() as i64).unsigned_abs()).max();
            println!("{name:8} {:>9} values, {} differ, max {} ULP", ours.len(), bad.len(), ulp.unwrap_or(0));
            if let Some((x, y)) = bad.first() {
                println!("         first: ours {x:e}, reference {y:e}");
            }
            ok &= bad.is_empty();
        }
        println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
        std::process::exit(!ok as i32);
    }
}
