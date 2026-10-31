// SPDX-License-Identifier: MPL-2.0
//! Runs a case with `W3XYP3`, `W3KTP3` and `W3SRCE` in the SPIR-V that rust-gpu built, and `VA` resident in
//! one device buffer. It only accepts a CPU Vulkan implementation: `AGENTS.md`, "Running on the GPU".
//! Usage: spirv_shel <spv> <case directory>.

use ash::vk;
use std::ffi::CStr;
use ww3_host::nml::read_case;
use ww3_host::shel::{Device, run};
use ww3_kernels::pro3::{Ktp3Params, Ktp3Point, Ktp3Tables, Xyp3Params, Xyp3Tables};
use ww3_kernels::real::Real;
use ww3_kernels::sin4::{Sin4Out, Spr4Out};
use ww3_kernels::srce::{SrceOut, SrceParams, SrcePoint, SrceTables, work_lens, work_small};

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
    srce: Pipe,
    va: Option<Buffer>,
    /// Tables that only need one upload, by pipeline.
    tables: [bool; 3],
    dispatches: u64,
    /// `VA` bytes moved, host to device and device to host.
    up: u64,
    down: u64,
}

#[derive(Clone, Copy)]
enum Which {
    Xyp3,
    Ktp3,
    Srce,
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
            let xyp3 = pipe(&device, module, c"xyp3", 41);
            let ktp3 = pipe(&device, module, c"ktp3", 26);
            let srce = pipe(&device, module, c"srce", 54);
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
            Self { device, memory, queue, cmd, fence, xyp3, ktp3, srce, va: None, tables: [false; 3], dispatches: 0, up: 0, down: 0 }
        }
    }

    /// A host-visible storage buffer.
    unsafe fn make(device: &ash::Device, memory: &vk::PhysicalDeviceMemoryProperties, size: usize) -> Buffer {
        unsafe {
            let info = vk::BufferCreateInfo::default().size(size as u64).usage(vk::BufferUsageFlags::STORAGE_BUFFER);
            let buffer = device.create_buffer(&info, None).unwrap();
            let req = device.get_buffer_memory_requirements(buffer);
            let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            let index = (0..memory.memory_type_count)
                .find(|t| {
                    req.memory_type_bits & (1 << t) != 0 && memory.memory_types[*t as usize].property_flags.contains(wanted)
                })
                .unwrap();
            let alloc = vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(index);
            let mem = device.allocate_memory(&alloc, None).unwrap();
            device.bind_buffer_memory(buffer, mem, 0).unwrap();
            let map = device.map_memory(mem, 0, req.size, vk::MemoryMapFlags::empty()).unwrap().cast();
            Buffer { buffer, memory: mem, size, map }
        }
    }

    unsafe fn bind(device: &ash::Device, set: vk::DescriptorSet, i: usize, buffer: vk::Buffer, size: usize) {
        unsafe {
            let info = [vk::DescriptorBufferInfo { buffer, offset: 0, range: size as u64 }];
            let write = vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(i as u32)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&info);
            device.update_descriptor_sets(&[write], &[]);
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
                let b = Self::make(device, memory, size);
                Self::bind(device, p.set, i, b.buffer, size);
                p.buffers[i] = Some(b);
            }
            p.buffers[i].as_ref().unwrap().map
        }
    }

    /// The one resident `VA`, bound into all three pipelines.
    fn va_bind(&mut self, size: usize) {
        if self.va.is_some() {
            return;
        }
        unsafe {
            let b = Self::make(&self.device, &self.memory, size);
            Self::bind(&self.device, self.xyp3.set, 40, b.buffer, size);
            Self::bind(&self.device, self.ktp3.set, 14, b.buffer, size);
            Self::bind(&self.device, self.srce.set, 26, b.buffer, size);
            self.va = Some(b);
        }
    }

    fn pipe_of(&mut self, w: Which) -> &mut Pipe {
        match w {
            Which::Xyp3 => &mut self.xyp3,
            Which::Ktp3 => &mut self.ktp3,
            Which::Srce => &mut self.srce,
        }
    }

    fn put(&mut self, w: Which, i: usize, data: &[u8]) {
        let (device, memory) = (self.device.clone(), self.memory);
        let p = self.pipe_of(w);
        unsafe {
            let map = Self::slot(&device, &memory, p, i, data.len().max(4));
            std::ptr::copy_nonoverlapping(data.as_ptr(), map, data.len());
        }
    }

    fn scratch(&mut self, w: Which, i: usize, size: usize) {
        let (device, memory) = (self.device.clone(), self.memory);
        let p = self.pipe_of(w);
        unsafe {
            Self::slot(&device, &memory, p, i, size.max(4));
        }
    }

    fn get<T>(&mut self, w: Which, i: usize, out: &mut [T]) {
        let p = self.pipe_of(w);
        let b = p.buffers[i].as_ref().unwrap();
        unsafe { std::ptr::copy_nonoverlapping(b.map, out.as_mut_ptr().cast::<u8>(), size_of_val(out)) }
    }

    fn run(&mut self, w: Which, n: usize) {
        let (cmd, queue, fence) = (self.cmd, self.queue, self.fence);
        let d = self.device.clone();
        let p = self.pipe_of(w);
        let (pipeline, layout, set) = (p.pipeline, p.layout, p.set);
        unsafe {
            d.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty()).unwrap();
            d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default()).unwrap();
            d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
            d.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::COMPUTE, layout, 0, &[set], &[]);
            d.cmd_dispatch(cmd, n.div_ceil(64) as u32, 1, 1);
            d.end_command_buffer(cmd).unwrap();
            let cmds = [cmd];
            d.queue_submit(queue, &[vk::SubmitInfo::default().command_buffers(&cmds)], fence).unwrap();
            d.wait_for_fences(&[fence], true, u64::MAX).unwrap();
            d.reset_fences(&[fence]).unwrap();
        }
        self.dispatches += 1;
    }
}

impl Device for Gpu {
    fn va_write(&mut self, va: &[Real]) {
        self.va_bind(size_of_val(va));
        let b = self.va.as_ref().unwrap();
        unsafe { std::ptr::copy_nonoverlapping(va.as_ptr().cast::<u8>(), b.map, size_of_val(va)) };
        self.up += size_of_val(va) as u64;
    }

    fn va_read(&mut self, va: &mut [Real]) {
        let b = self.va.as_ref().unwrap();
        unsafe { std::ptr::copy_nonoverlapping(b.map, va.as_mut_ptr().cast::<u8>(), size_of_val(va)) };
        self.down += size_of_val(va) as u64;
    }

    fn whitecap_read(&mut self, w: &mut [Real]) {
        if self.srce.buffers[50].is_some() {
            self.get(Which::Srce, 50, w);
        }
    }

    fn xyp3(&mut self, p: &Xyp3Params, t: &Xyp3Tables, dtg: Real, yfirst: bool, nspec: usize) {
        let (nx, ny, nsea) = (p.nx as usize, p.ny as usize, p.nsea as usize);
        let w = Which::Xyp3;
        self.put(w, 0, bytes(std::slice::from_ref(p)));
        self.put(w, 1, bytes(&[nspec as i32, yfirst as i32]));
        self.put(w, 2, bytes(&[dtg]));
        self.put(w, 21, bytes(t.cx));
        self.put(w, 22, bytes(t.cy));
        if !self.tables[0] {
            self.tables[0] = true;
            let tables: [(usize, &[u8]); 20] = [
                (3, bytes(t.sig)), (4, bytes(t.ecos)), (5, bytes(t.esin)), (6, bytes(t.mapsf)), (7, bytes(t.mapsta)),
                (8, bytes(t.mapstx)), (9, bytes(t.mapaxy)), (10, bytes(t.mapcxy)), (11, bytes(t.mapx2)),
                (12, bytes(t.mapy2)), (13, bytes(t.maptrn)), (14, bytes(t.clats)), (15, bytes(t.dpdx)),
                (16, bytes(t.dpdy)), (17, bytes(t.dqdx)), (18, bytes(t.dqdy)), (19, bytes(t.gsqrt)), (20, bytes(t.cg)),
                (23, bytes(t.atrnx)), (24, bytes(t.atrny)),
            ];
            for (i, b) in tables {
                self.put(w, i, b);
            }
            self.scratch(w, 25, 4 * (nx + 3) * ny * nspec);
            for i in 26..34 {
                self.scratch(w, i, 4 * nsea * nspec);
            }
            self.scratch(w, 34, 4 * (nx + 1) * ny * nspec);
            self.scratch(w, 35, 4 * (nx + 1) * ny * nspec);
            self.scratch(w, 36, 4 * (nx + 1) * ny * nspec);
            self.scratch(w, 37, 4 * nx * ny * nspec);
            self.scratch(w, 38, 4 * (nx + 3) * ny * nspec);
            self.scratch(w, 39, 4 * (nx + 1) * ny * nspec);
        }
        self.run(w, nspec);
    }

    fn ktp3(&mut self, p: &Ktp3Params, t: &Ktp3Tables, pts: &[Ktp3Point], active: &[u32], cg: &[Real], wn: &[Real]) {
        let (nk, nth, n) = (p.nk as usize, p.nth as usize, pts.len());
        let nk2 = nk + 2;
        let w = Which::Ktp3;
        self.put(w, 0, bytes(std::slice::from_ref(p)));
        self.put(w, 1, bytes(pts));
        if !self.tables[1] {
            self.tables[1] = true;
            let tables: [&[u8]; 12] = [
                bytes(t.sig), bytes(t.dsip), bytes(t.ecos), bytes(t.esin), bytes(t.ec2), bytes(t.esc), bytes(t.es2),
                bytes(t.mapwn), bytes(t.mapth2), bytes(t.mapwn2), bytes(cg), bytes(wn),
            ];
            for (i, b) in tables.iter().enumerate() {
                self.put(w, 2 + i, b);
            }
            self.put(w, 25, bytes(active));
            self.scratch(w, 15, 4 * nk2 * n);
            self.scratch(w, 16, 4 * nk * n);
            self.scratch(w, 17, 4 * nk * n);
            self.scratch(w, 18, 4 * nth * n);
            self.scratch(w, 19, 4 * nk2 * (nth + 3) * n);
            self.scratch(w, 20, 4 * nk2 * (nth + 1) * n);
            self.scratch(w, 21, 4 * nk2 * (nth + 2) * n);
            self.scratch(w, 22, 4 * nk2 * (nth + 1) * n);
            self.scratch(w, 23, 4 * nk2 * nth * n);
            self.scratch(w, 24, 4 * nk2 * (nth + 1) * n);
        }
        self.run(w, n);
    }

    fn srce(
        &mut self,
        p: &SrceParams,
        t: &SrceTables,
        pts: &[SrcePoint],
        active: &[u32],
        wn1: &[Real],
        cg1: &[Real],
        out: &mut [SrceOut],
    ) {
        let (n, nspec) = (pts.len(), p.nspec as usize);
        let w = Which::Srce;
        self.put(w, 23, bytes(pts));
        if !self.tables[2] {
            self.tables[2] = true;
            let (l_snl, l_sds, l_ids) = work_lens(p);
            let tables: [(usize, &[u8]); 26] = [
                (0, bytes(std::slice::from_ref(p))), (1, bytes(t.sig)), (2, bytes(t.sig2)), (3, bytes(t.dden)),
                (4, bytes(t.dden2)), (5, bytes(t.ec2)), (6, bytes(t.es2)), (7, bytes(t.esc)), (8, bytes(t.ecos)),
                (9, bytes(t.esin)), (10, bytes(t.af11)), (11, bytes(t.pm)), (12, bytes(t.ic)),
                (13, bytes(t.satindices)), (14, bytes(t.satweights)), (15, bytes(t.cumulw)), (16, bytes(t.iktab)),
                (17, bytes(t.dcki)), (18, bytes(t.qbi)), (19, bytes(t.fwtable)), (20, bytes(t.tauhft)),
                (21, bytes(t.tauhft2)), (22, bytes(t.taut)), (24, bytes(wn1)), (25, bytes(cg1)), (53, bytes(active)),
            ];
            for (i, b) in tables {
                self.put(w, i, b);
            }
            self.scratch(w, 27, n * size_of::<SrceOut>());
            for i in 28..46 {
                self.scratch(w, i, n * nspec * 4);
            }
            self.scratch(w, 46, n * l_snl * 4);
            self.scratch(w, 47, n * l_sds * 4);
            self.scratch(w, 48, n * l_ids * 4);
            self.scratch(w, 49, n * work_small(p) * 4);
            self.scratch(w, 50, n * 16);
            self.scratch(w, 51, n * size_of::<Sin4Out>());
            self.scratch(w, 52, n * size_of::<Spr4Out>());
        }
        self.run(w, n);
        self.get(w, 27, out);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (spv, dir) = (args.next().expect("spv"), args.next().expect("case directory"));
    let mut gpu = Gpu::new(&std::fs::read(&spv).unwrap());
    let (m, r) = read_case(&dir);
    run(&m, r, &dir, Some(&mut gpu));
    println!("{} dispatches", gpu.dispatches);
    println!("VA moved: {} bytes to the device, {} bytes to the host", gpu.up, gpu.down);
}
