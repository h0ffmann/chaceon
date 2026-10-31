// SPDX-License-Identifier: MPL-2.0
//! Runs a case with `W3XYP3` and `W3KTP3` in the SPIR-V that rust-gpu built. It only accepts a CPU Vulkan
//! implementation: `AGENTS.md`, "Running on the GPU". Usage: spirv_shel <spv> <case directory>.

use ash::vk;
use std::ffi::CStr;
use ww3_host::nml::read_case;
use ww3_host::shel::{Device, run};
use ww3_kernels::pro3::{Ktp3Params, Ktp3Point, Ktp3Tables, Xyp3Params, Xyp3Tables};
use ww3_kernels::real::Real;

fn bytes<T>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), size_of_val(v)) }
}

struct Buffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: usize,
    map: *mut u8,
}

/// One compute pipeline and its storage buffers, which grow when a call needs more.
struct Pipe {
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    set: vk::DescriptorSet,
    buffers: Vec<Option<Buffer>>,
}

struct Gpu {
    device: ash::Device,
    memory: vk::PhysicalDeviceMemoryProperties,
    queue: vk::Queue,
    cmd: vk::CommandBuffer,
    fence: vk::Fence,
    xyp3: Pipe,
    ktp3: Pipe,
    dispatches: u64,
}

unsafe fn pipe(device: &ash::Device, module: vk::ShaderModule, name: &CStr, n: u32) -> Pipe {
    unsafe {
        let bindings: Vec<_> = (0..n)
            .map(|i| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(i)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
            })
            .collect();
        let dsl = device
            .create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)
            .unwrap();
        let layouts = [dsl];
        let layout = device
            .create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts), None)
            .unwrap();
        let stage =
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::COMPUTE).module(module).name(name);
        let pipeline = device
            .create_compute_pipelines(
                vk::PipelineCache::null(),
                &[vk::ComputePipelineCreateInfo::default().stage(stage).layout(layout)],
                None,
            )
            .unwrap()[0];
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: n }];
        let pool = device
            .create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None)
            .unwrap();
        let set = device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts),
            )
            .unwrap()[0];
        Pipe { pipeline, layout, set, buffers: (0..n).map(|_| None).collect() }
    }
}

impl Gpu {
    fn new(code: &[u8]) -> Self {
        let words: Vec<u32> = code.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
        unsafe {
            let entry = ash::Entry::load().expect("Vulkan loader");
            let app = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_2);
            let instance = entry.create_instance(&vk::InstanceCreateInfo::default().application_info(&app), None).unwrap();
            let phys = instance
                .enumerate_physical_devices()
                .unwrap()
                .into_iter()
                .find(|d| instance.get_physical_device_properties(*d).device_type == vk::PhysicalDeviceType::CPU)
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
            let queues = [vk::DeviceQueueCreateInfo::default().queue_family_index(family).queue_priorities(&priorities)];
            let device = instance
                .create_device(phys, &vk::DeviceCreateInfo::default().queue_create_infos(&queues).enabled_features(&features), None)
                .unwrap();
            let memory = instance.get_physical_device_memory_properties(phys);
            let module = device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None).unwrap();
            let xyp3 = pipe(&device, module, c"xyp3", 40);
            let ktp3 = pipe(&device, module, c"ktp3", 25);
            let pool = device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(family)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .unwrap();
            let cmd = device
                .allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(1))
                .unwrap()[0];
            let fence = device.create_fence(&vk::FenceCreateInfo::default(), None).unwrap();
            let queue = device.get_device_queue(family, 0);
            Self { device, memory, queue, cmd, fence, xyp3, ktp3, dispatches: 0 }
        }
    }

    /// Makes binding `i` at least `size` bytes, and rebinds it when it grew.
    unsafe fn slot(device: &ash::Device, memory: &vk::PhysicalDeviceMemoryProperties, p: &mut Pipe, i: usize, size: usize) -> *mut u8 {
        unsafe {
            if p.buffers[i].as_ref().is_none_or(|b| b.size < size) {
                if let Some(b) = p.buffers[i].take() {
                    device.destroy_buffer(b.buffer, None);
                    device.free_memory(b.memory, None);
                }
                let info = vk::BufferCreateInfo::default().size(size as u64).usage(vk::BufferUsageFlags::STORAGE_BUFFER);
                let buffer = device.create_buffer(&info, None).unwrap();
                let req = device.get_buffer_memory_requirements(buffer);
                let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
                let index = (0..memory.memory_type_count)
                    .find(|t| {
                        req.memory_type_bits & (1 << t) != 0
                            && memory.memory_types[*t as usize].property_flags.contains(wanted)
                    })
                    .unwrap();
                let alloc = vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(index);
                let mem = device.allocate_memory(&alloc, None).unwrap();
                device.bind_buffer_memory(buffer, mem, 0).unwrap();
                let map = device.map_memory(mem, 0, req.size, vk::MemoryMapFlags::empty()).unwrap().cast();
                p.buffers[i] = Some(Buffer { buffer, memory: mem, size, map });
                let info = [vk::DescriptorBufferInfo { buffer, offset: 0, range: size as u64 }];
                let write = vk::WriteDescriptorSet::default()
                    .dst_set(p.set)
                    .dst_binding(i as u32)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(&info);
                device.update_descriptor_sets(&[write], &[]);
            }
            p.buffers[i].as_ref().unwrap().map
        }
    }

    fn put(&mut self, kt: bool, i: usize, data: &[u8]) {
        let p = if kt { &mut self.ktp3 } else { &mut self.xyp3 };
        unsafe {
            let map = Self::slot(&self.device, &self.memory, p, i, data.len().max(4));
            std::ptr::copy_nonoverlapping(data.as_ptr(), map, data.len());
        }
    }

    fn scratch(&mut self, kt: bool, i: usize, size: usize) {
        let p = if kt { &mut self.ktp3 } else { &mut self.xyp3 };
        unsafe {
            Self::slot(&self.device, &self.memory, p, i, size.max(4));
        }
    }

    fn get(&self, kt: bool, i: usize, out: &mut [Real]) {
        let p = if kt { &self.ktp3 } else { &self.xyp3 };
        let b = p.buffers[i].as_ref().unwrap();
        unsafe { std::ptr::copy_nonoverlapping(b.map, out.as_mut_ptr().cast(), size_of_val(out)) }
    }

    fn run(&mut self, kt: bool, n: usize) {
        let p = if kt { &self.ktp3 } else { &self.xyp3 };
        let d = &self.device;
        unsafe {
            d.reset_command_buffer(self.cmd, vk::CommandBufferResetFlags::empty()).unwrap();
            d.begin_command_buffer(self.cmd, &vk::CommandBufferBeginInfo::default()).unwrap();
            d.cmd_bind_pipeline(self.cmd, vk::PipelineBindPoint::COMPUTE, p.pipeline);
            d.cmd_bind_descriptor_sets(self.cmd, vk::PipelineBindPoint::COMPUTE, p.layout, 0, &[p.set], &[]);
            d.cmd_dispatch(self.cmd, n.div_ceil(64) as u32, 1, 1);
            d.end_command_buffer(self.cmd).unwrap();
            let cmds = [self.cmd];
            d.queue_submit(self.queue, &[vk::SubmitInfo::default().command_buffers(&cmds)], self.fence).unwrap();
            d.wait_for_fences(&[self.fence], true, u64::MAX).unwrap();
            d.reset_fences(&[self.fence]).unwrap();
        }
        self.dispatches += 1;
    }
}

impl Device for Gpu {
    fn xyp3(&mut self, p: &Xyp3Params, t: &Xyp3Tables, dtg: Real, yfirst: bool, comps: usize, vq: &mut [Real]) {
        let (nx, ny, nsea) = (p.nx as usize, p.ny as usize, p.nsea as usize);
        self.put(false, 0, bytes(std::slice::from_ref(p)));
        self.put(false, 1, bytes(&[comps as i32, yfirst as i32]));
        self.put(false, 2, bytes(&[dtg]));
        let tables: [&[u8]; 22] = [
            bytes(t.sig), bytes(t.ecos), bytes(t.esin), bytes(t.mapsf), bytes(t.mapsta), bytes(t.mapstx),
            bytes(t.mapaxy), bytes(t.mapcxy), bytes(t.mapx2), bytes(t.mapy2), bytes(t.maptrn), bytes(t.clats),
            bytes(t.dpdx), bytes(t.dpdy), bytes(t.dqdx), bytes(t.dqdy), bytes(t.gsqrt), bytes(t.cg), bytes(t.cx),
            bytes(t.cy), bytes(t.atrnx), bytes(t.atrny),
        ];
        for (i, b) in tables.iter().enumerate() {
            self.put(false, 3 + i, b);
        }
        self.put(false, 25, bytes(vq));
        for i in 26..34 {
            self.scratch(false, i, 4 * nsea * comps);
        }
        self.scratch(false, 34, 4 * (nx + 1) * ny * comps);
        self.scratch(false, 35, 4 * (nx + 1) * ny * comps);
        self.scratch(false, 36, 4 * (nx + 1) * ny * comps);
        self.scratch(false, 37, 4 * nx * ny * comps);
        self.scratch(false, 38, 4 * (nx + 3) * ny * comps);
        self.scratch(false, 39, 4 * (nx + 1) * ny * comps);
        self.run(false, comps);
        self.get(false, 25, vq);
    }

    fn ktp3(&mut self, p: &Ktp3Params, t: &Ktp3Tables, pts: &[Ktp3Point], cg: &[Real], wn: &[Real], va: &mut [Real]) {
        if pts.is_empty() {
            return;
        }
        let (nk, nth, n) = (p.nk as usize, p.nth as usize, pts.len());
        let nk2 = nk + 2;
        self.put(true, 0, bytes(std::slice::from_ref(p)));
        self.put(true, 1, bytes(pts));
        let tables: [&[u8]; 12] = [
            bytes(t.sig), bytes(t.dsip), bytes(t.ecos), bytes(t.esin), bytes(t.ec2), bytes(t.esc), bytes(t.es2),
            bytes(t.mapwn), bytes(t.mapth2), bytes(t.mapwn2), bytes(cg), bytes(wn),
        ];
        for (i, b) in tables.iter().enumerate() {
            self.put(true, 2 + i, b);
        }
        self.put(true, 14, bytes(va));
        self.scratch(true, 15, 4 * nk2 * n);
        self.scratch(true, 16, 4 * nk * n);
        self.scratch(true, 17, 4 * nk * n);
        self.scratch(true, 18, 4 * nth * n);
        self.scratch(true, 19, 4 * nk2 * (nth + 3) * n);
        self.scratch(true, 20, 4 * nk2 * (nth + 1) * n);
        self.scratch(true, 21, 4 * nk2 * (nth + 2) * n);
        self.scratch(true, 22, 4 * nk2 * (nth + 1) * n);
        self.scratch(true, 23, 4 * nk2 * nth * n);
        self.scratch(true, 24, 4 * nk2 * (nth + 1) * n);
        self.run(true, n);
        self.get(true, 14, va);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (spv, dir) = (args.next().expect("spv"), args.next().expect("case directory"));
    let mut gpu = Gpu::new(&std::fs::read(&spv).unwrap());
    let (m, r) = read_case(&dir);
    run(&m, r, &dir, Some(&mut gpu));
    println!("{} dispatches", gpu.dispatches);
}
