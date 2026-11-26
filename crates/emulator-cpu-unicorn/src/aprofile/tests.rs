use super::*;
fn cpu() -> Core {
    let raw = crate::test_support::library()
        .open(Model::ArmV8A, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0x1000, 0x4000, 7).unwrap();
    cpu.write_register(Register::Pstate, &0x3cdu64.to_le_bytes())
        .unwrap();
    cpu
}
fn value(cpu: &dyn Cpu, r: Register) -> u64 {
    let mut b = [0; 8];
    cpu.read_register(r, &mut b).unwrap();
    u64::from_le_bytes(b)
}
fn code(cpu: &Core, words: &[u32]) {
    cpu.write_memory(
        0x1000,
        &words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn run(cpu: &Core, begin: u64, budget: usize) -> Exit {
    cpu.run(Run {
        begin,
        until: None,
        instructions: Some(budget),
        watchdog_us: 0,
    })
    .unwrap()
}
#[test]
fn interactive_policy_is_forwarded_and_keeps_exact_instruction_limits() {
    let cpu = cpu();
    cpu.set_execution_policy(ExecutionPolicy::Interactive)
        .unwrap();
    code(&cpu, &[0x91000400, 0x17ffffff]);
    assert!(matches!(run(&cpu, 0x1000, 1), Exit::Stopped));
    assert_eq!(cpu.instruction_progress(), Some(1));
    assert_eq!(value(&cpu, Register::X(0)), 1);
}
#[test]
fn every_budget_boundary_matches_instruction_entries() {
    let cpu = cpu();
    code(
        &cpu,
        &[0x91000400, 0x91000400, 0x91000400, 0x91000400, 0x17fffffc],
    );
    for n in 0..83 {
        cpu.write_register(Register::Pc, &0x1000u64.to_le_bytes())
            .unwrap();
        cpu.write_register(Register::X(0), &0u64.to_le_bytes())
            .unwrap();
        run(&cpu, 0x1000, n);
        assert_eq!(cpu.instruction_progress(), Some(n as u64));
        assert_eq!(value(&cpu, Register::Pc), 0x1000 + (n % 5) as u64 * 4);
        assert_eq!(value(&cpu, Register::X(0)), (n - n / 5) as u64);
    }
}
#[test]
fn framework_exit_precedes_entry_and_preserves_exact_budget() {
    let cpu = cpu();
    code(&cpu, &[0x91000400, 0x91000400, 0xd53be040, 0x91000400]); // MRS x0,CNTVCT_EL0
    assert!(matches!(run(&cpu, 0x1000, 2), Exit::Stopped));
    assert_eq!(cpu.instruction_progress(), Some(2));
    assert!(matches!(
        run(&cpu, 0x1000, 3),
        Exit::BeforeInstruction(0xd53be040)
    ));
    assert_eq!(cpu.instruction_progress(), Some(2));
    assert_eq!(value(&cpu, Register::Pc), 0x1008);
}

struct Device<'a> {
    cpu: &'a Core,
    reads: Vec<u64>,
    writes: Vec<u64>,
    tighten: bool,
}
unsafe fn read(_: u64, _: u32, data: *mut c_void) -> u64 {
    let d = unsafe { &mut *data.cast::<Device<'_>>() };
    d.reads.push(d.cpu.instruction_progress().unwrap());
    if d.tighten {
        d.cpu.tighten_instruction_limit(3).unwrap();
    }
    42
}
unsafe fn write(_: u64, _: u32, _: u64, data: *mut c_void) {
    let d = unsafe { &mut *data.cast::<Device<'_>>() };
    d.writes.push(d.cpu.instruction_progress().unwrap());
}
#[test]
fn mmio_entries_and_tightened_deadlines_are_exact() {
    for tighten in [false, true] {
        let cpu = cpu();
        code(
            &cpu,
            &[
                0x91000442, 0x91000442, 0xb9400020, 0x91000442, 0xb9000020, 0x91000442, 0x14000000,
            ],
        );
        cpu.write_register(Register::X(1), &0x8000u64.to_le_bytes())
            .unwrap();
        let mut d = Device {
            cpu: &cpu,
            reads: vec![],
            writes: vec![],
            tighten,
        };
        unsafe {
            cpu.map_mmio(
                0x8000,
                0x1000,
                Mmio {
                    read,
                    write,
                    data: (&mut d as *mut Device<'_>).cast(),
                },
            )
            .unwrap()
        }
        run(&cpu, 0x1000, 6);
        assert_eq!(d.reads, [3]);
        assert_eq!(value(&cpu, Register::X(0)), 42);
        assert_eq!(d.writes, if tighten { vec![] } else { vec![5] });
        assert_eq!(
            cpu.instruction_progress(),
            Some(if tighten { 3 } else { 6 })
        );
        assert_eq!(
            value(&cpu, Register::Pc),
            if tighten { 0x100c } else { 0x1018 }
        );
    }
}
struct HighAddressDevice {
    reads: Vec<(u64, u32)>,
}
unsafe fn high_address_read(offset: u64, size: u32, data: *mut c_void) -> u64 {
    let device = unsafe { &mut *data.cast::<HighAddressDevice>() };
    device.reads.push((offset, size));
    0x1234_5678
}
unsafe fn high_address_write(_: u64, _: u32, _: u64, _: *mut c_void) {}

#[test]
fn x100vi_el1_mmio_read_uses_scr_rw_and_reaches_callback() {
    let raw = crate::test_support::library()
        .open(Model::ArmV8A, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0, 0x68000000, 7).unwrap(); // X100VI DRAM extent.
    let system_value = |encoding| {
        value(
            &cpu,
            Register::Arm64System(arm::SystemRegister::from_encoding(encoding)),
        )
    };
    assert_eq!(system_value(0xf088) & 0x400, 0x400);
    assert_eq!(system_value(0xe088) & 0x8000_0000, 0x8000_0000);
    cpu.write_register(Register::Pstate, &0x3c4u64.to_le_bytes())
        .unwrap();
    let system = |encoding: u16, value: u64| {
        cpu.write_register(
            Register::Arm64System(arm::SystemRegister::from_encoding(encoding)),
            &value.to_le_bytes(),
        )
        .unwrap();
    };
    cpu.write_memory(0x108000, &0x609u64.to_le_bytes()).unwrap();
    cpu.write_memory(0x108018, &0x109003u64.to_le_bytes())
        .unwrap();
    cpu.write_memory(
        0x109000 + 0x1d0 * 8,
        &0x0010_0000_fa00_0601u64.to_le_bytes(),
    )
    .unwrap();
    system(0xc100, 0x108000); // TTBR0_EL1.
    system(0xc102, 0x1a59c251c); // 36-bit VA, 4 KiB granule.
    system(0xc510, 0xff4404); // MAIR_EL1.
    system(0xc080, 0x30d00801); // SCTLR_EL1 at the observed failure.
    // LDR W0, [X0] from the observed boot stop.
    cpu.write_memory(0x9ed88, &0xb9400000u32.to_le_bytes())
        .unwrap();
    cpu.write_register(Register::X(0), &0xfa090400u64.to_le_bytes())
        .unwrap();
    let mut device = HighAddressDevice { reads: vec![] };
    unsafe {
        cpu.map_mmio(
            0xfa000000,
            0x100000,
            Mmio {
                read: high_address_read,
                write: high_address_write,
                data: (&mut device as *mut HighAddressDevice).cast(),
            },
        )
        .unwrap()
    }
    assert_eq!(cpu.translate(0xfa090400, Access::Read).unwrap(), 0xfa090400);
    run(&cpu, 0x9ed88, 1);
    assert_eq!(device.reads, [(0x90400, 4)]);
    assert_eq!(value(&cpu, Register::X(0)), 0x1234_5678);
}
unsafe fn intercept(_: u64, _: u32, data: *mut c_void) {
    unsafe { &*data.cast::<Core>() }
        .stop_before_instruction()
        .unwrap();
}
#[test]
fn observer_intercepts_before_entry_and_effect() {
    let cpu = cpu();
    code(&cpu, &[0x91000400, 0x91000400, 0x91000400]);
    unsafe {
        cpu.install_hook(Hook::Instruction {
            range: HookRange::Inclusive {
                begin: 0x1004,
                end: 0x1004,
            },
            callback: intercept,
            data: (&cpu as *const Core).cast_mut().cast(),
        })
        .unwrap();
    }
    run(&cpu, 0x1000, 3);
    assert_eq!(cpu.instruction_progress(), Some(1));
    assert_eq!(value(&cpu, Register::X(0)), 1);
    assert_eq!(value(&cpu, Register::Pc), 0x1004);
}
#[test]
fn irq_unmask_stops_before_the_following_instruction() {
    let cpu = cpu();
    code(&cpu, &[0xd50342ff, 0x91000400]); // DAIFClr #2; ADD x0,x0,#1.
    let sys = |n| Register::Arm64System(arm::SystemRegister::from_encoding(n));
    cpu.write_register(sys(0xf088), &0x400u64.to_le_bytes())
        .unwrap();
    cpu.write_register(Register::Pstate, &0x3c5u64.to_le_bytes())
        .unwrap();
    cpu.write_register(sys(0xc600), &0x2000u64.to_le_bytes())
        .unwrap();
    cpu.set_interrupt(Interrupt::Irq, true).unwrap();
    run(&cpu, 0x1000, 2);
    assert_eq!(cpu.instruction_progress(), Some(1));
    assert_eq!(value(&cpu, Register::Pc), 0x2280);
    assert_eq!(value(&cpu, sys(0xc201)), 0x1004);
    assert_eq!(value(&cpu, Register::X(0)), 0);
}
#[test]
fn host_code_write_invalidates_every_core_sharing_backing() {
    use fujimod_emulator_cpu::memory::Backing;
    let memory = Memory::default();
    let backing = Backing::aligned_zeros(0x4000, 4096).unwrap();
    let open = || {
        let raw = crate::test_support::library()
            .open(Model::ArmV8A, 1 << 20)
            .unwrap();
        let cpu = Core::new(raw, memory.clone()).unwrap();
        unsafe {
            cpu.map_physical(
                0x1000,
                0x1000,
                0x4000,
                7,
                NonNull::new(backing.as_ptr()).unwrap(),
            )
            .unwrap()
        };
        cpu
    };
    let a = open();
    let b = open();
    code(&a, &[0x91000400, 0x14000000]);
    run(&a, 0x1000, 1);
    run(&b, 0x1000, 1);
    code(&a, &[0x91001400, 0x14000000]);
    run(&b, 0x1000, 1);
    assert_eq!(value(&b, Register::X(0)), 6);
}

#[test]
fn snapshots_restore_pending_interrupt_inputs() {
    let cpu = cpu();
    code(&cpu, &[0xd50342ff, 0x91000400]);
    let sys = |n| Register::Arm64System(arm::SystemRegister::from_encoding(n));
    cpu.write_register(sys(0xf088), &0x400u64.to_le_bytes())
        .unwrap();
    cpu.write_register(sys(0xc600), &0x2000u64.to_le_bytes())
        .unwrap();
    cpu.write_register(Register::Pstate, &0x3c5u64.to_le_bytes())
        .unwrap();
    cpu.set_interrupt(Interrupt::Irq, true).unwrap();
    let saved = cpu.snapshot(None).unwrap();
    cpu.set_interrupt(Interrupt::Irq, false).unwrap();
    cpu.restore(&*saved).unwrap();
    run(&cpu, 0x1000, 2);
    assert_eq!(value(&cpu, Register::Pc), 0x2280);
    assert_eq!(cpu.instruction_progress(), Some(1));
}

#[test]
fn completed_store_observers_see_ram_and_can_stop_before_the_next_entry() {
    unsafe fn stop(_: Access, address: u64, _: i32, _: i64, data: *mut c_void) {
        let cpu = unsafe { &*data.cast::<Core>() };
        let mut bytes = [0; 4];
        cpu.read_memory(address, &mut bytes).unwrap();
        assert_eq!(u32::from_le_bytes(bytes), 42);
        cpu.tighten_instruction_limit(cpu.instruction_progress().unwrap())
            .unwrap();
    }
    let cpu = cpu();
    // Two stores to data-only pages ensure cached fast-path stores stay observed.
    code(
        &cpu,
        &[0x52800540, 0xb9000020, 0xb9000020, 0x91000400, 0x14000000],
    );
    cpu.write_register(Register::X(1), &0x3000u64.to_le_bytes())
        .unwrap();
    let hook = unsafe {
        cpu.install_hook(Hook::CompletedStore {
            range: HookRange::Inclusive {
                begin: 0x3000,
                end: 0x3003,
            },
            callback: stop,
            data: (&cpu as *const Core).cast_mut().cast(),
        })
    }
    .unwrap();
    run(&cpu, 0x1000, 20);
    assert_eq!(cpu.instruction_progress(), Some(2));
    assert_eq!(value(&cpu, Register::Pc), 0x1008);
    run(&cpu, 0x1008, 20);
    assert_eq!(cpu.instruction_progress(), Some(1));
    assert_eq!(value(&cpu, Register::Pc), 0x100c);
    cpu.remove_hook(hook).unwrap();
    run(&cpu, 0x100c, 1);
    assert_eq!(value(&cpu, Register::X(0)), 43);
}

#[test]
fn armv7a_checkpoint_reports_its_backing_extents_and_imports_into_a_fresh_core() {
    let library = crate::test_support::library();
    let open = || {
        let raw = library.open(Model::CortexA7, 1 << 20).unwrap();
        let cpu = Core::new(raw, Memory::default()).unwrap();
        cpu.map_zeroed(0x1000, 0x4000, 7).unwrap();
        cpu
    };
    let set = |cpu: &Core, n: usize, value: u32| {
        cpu.write_register(arm::CORE[n], &value.to_le_bytes())
            .unwrap()
    };
    let cpu = open();
    assert_eq!(
        cpu.checkpoint_backings()
            .unwrap()
            .iter()
            .map(|b| b.1)
            .collect::<Vec<_>>(),
        [0x4000]
    );
    set(&cpu, 3, 0x1234);
    cpu.set_interrupt(Interrupt::Irq, true).unwrap();
    let state = cpu.state().unwrap();
    assert_eq!(cpu.state().unwrap(), state);
    let saved = cpu.snapshot(None).unwrap();
    let bytes = saved.export().unwrap();
    set(&cpu, 3, 0x9999);
    cpu.set_interrupt(Interrupt::Irq, false).unwrap();
    assert_ne!(cpu.state().unwrap(), state);
    cpu.restore(&*saved).unwrap();
    assert_eq!(cpu.state().unwrap(), state);
    // A fresh processor adopts the exported state, pending IRQ included.
    let fresh = open();
    assert_ne!(fresh.state().unwrap(), state);
    let imported = fresh.import_snapshot(&bytes).unwrap();
    fresh.restore(&*imported).unwrap();
    assert_eq!(fresh.state().unwrap(), state);
    assert!(fresh.import_snapshot(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn high_code_retains_coherence_after_host_and_guest_writes() {
    let cpu = cpu();
    let high = 0x1_0000_0000u64;
    cpu.map_zeroed(high, 0x4000, 7).unwrap();
    let set = |word: u32| cpu.write_memory(high, &word.to_le_bytes()).unwrap();
    set(0xd2800020); // mov x0,#1
    run(&cpu, high, 1);
    assert_eq!(value(&cpu, Register::X(0)), 1);
    set(0xd2800040); // mov x0,#2
    run(&cpu, high, 1);
    assert_eq!(value(&cpu, Register::X(0)), 2);
    code(&cpu, &[0xb9000022]); // str w2,[x1]
    cpu.write_register(Register::X(1), &high.to_le_bytes())
        .unwrap();
    cpu.write_register(Register::X(2), &0xd2800060u64.to_le_bytes())
        .unwrap();
    run(&cpu, 0x1000, 1);
    run(&cpu, high, 1);
    assert_eq!(value(&cpu, Register::X(0)), 3);
}

#[test]
fn high_code_aliases_keep_shared_backing_dependencies() {
    use fujimod_emulator_cpu::memory::Backing;
    let memory = Memory::default();
    let backing = Backing::aligned_zeros(0x4000, 4096).unwrap();
    let raw = crate::test_support::library()
        .open(Model::ArmV8A, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, memory).unwrap();
    let high = 0x1_0000_0000u64;
    for address in [0x1000, high] {
        unsafe {
            cpu.map_physical(
                address,
                high,
                0x4000,
                7,
                NonNull::new(backing.as_ptr()).unwrap(),
            )
            .unwrap()
        }
    }
    cpu.write_memory(0x1000, &0xd2800020u32.to_le_bytes())
        .unwrap();
    run(&cpu, high, 1);
    assert_eq!(value(&cpu, Register::X(0)), 1);
    cpu.write_memory(0x1000, &0xd2800040u32.to_le_bytes())
        .unwrap();
    run(&cpu, high, 1);
    assert_eq!(value(&cpu, Register::X(0)), 2);
}

fn a32_code_rewritten_in_place_runs_the_new_code(base: u64) {
    // Code runs at `base`, then a routine elsewhere overwrites it in place with
    // a CPU store and branches back: the new code must run.
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0, 0x8000, 7).unwrap();
    cpu.write_register(arm::CPSR, &0xd3u64.to_le_bytes())
        .unwrap();
    let words = |address: u64, words: &[u32]| {
        let bytes = words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>();
        cpu.write_memory(address, &bytes).unwrap();
    };
    let routine = base + 0x2000;
    let branch = |from: u64, to: u64| {
        0xea00_0000 | (((to as i64 - from as i64 - 8) >> 2) as u32 & 0x00ff_ffff)
    };
    // base: mov r0, #1; b routine
    words(base, &[0xe3a00001, branch(base + 4, routine)]);
    // routine: ldr r1, [pc, #8]; ldr r2, [pc, #8]; str r1, [r2]; bx r2; mov r0, #2; base
    words(
        routine,
        &[
            0xe59f1008,
            0xe59f2008,
            0xe5821000,
            0xe12fff12,
            0xe3a00002,
            base as u32,
        ],
    );
    run(&cpu, base, 7);
    assert_eq!(value(&cpu, arm::CORE[0]), 2);
}

#[test]
fn graph_mpidr_policy_preserves_interactive_exits_after_exact_runs() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0x1000, 0x1000, 7).unwrap();
    cpu.write_register(arm::CPSR, &0xd3u32.to_le_bytes()).unwrap();
    let mpidr = 0xee10_0fb0;
    code(&cpu, &[mpidr]);
    cpu.set_armv7a_affinity(0x8000_0001).unwrap();

    cpu.select_armv7a_affinity_policy(ExecutionPolicy::Exact).unwrap();
    assert!(matches!(run(&cpu, 0x1000, 1), Exit::Stopped));
    assert_eq!(value(&cpu, arm::CORE[0]), 0x8000_0001);

    cpu.select_armv7a_affinity_policy(ExecutionPolicy::Interactive).unwrap();
    assert!(matches!(run(&cpu, 0x1000, 1), Exit::BeforeInstruction(word) if word == mpidr));
    assert_eq!(cpu.instruction_progress(), Some(0));

    cpu.select_armv7a_affinity_policy(ExecutionPolicy::Exact).unwrap();
    assert!(matches!(run(&cpu, 0x1000, 1), Exit::Stopped));
    assert_eq!(cpu.instruction_progress(), Some(1));
}

#[test]
fn a32_code_rewritten_in_place_at_physical_page_zero_runs_the_new_code() {
    a32_code_rewritten_in_place_runs_the_new_code(0);
}

#[test]
fn a32_code_rewritten_in_place_above_page_zero_runs_the_new_code() {
    a32_code_rewritten_in_place_runs_the_new_code(0x4000);
}

#[test]
fn code_rewritten_again_while_a_peer_has_not_flushed_runs_the_new_code() {
    // Two processors map one RAM backing. The peer never runs, so it keeps its
    // queued invalidation of the first rewrite; later rewrites of code the
    // running processor translated again must still reach that processor.
    use crate::memory::SharedMemory;
    use fujimod_emulator_cpu::memory::Backing;
    let shared = SharedMemory::default();
    let backing = Backing::aligned_zeros(0x4000, 4096).unwrap();
    let core = |memory| {
        let raw = crate::test_support::library()
            .open(Model::CortexA7, 1 << 20)
            .unwrap();
        let cpu = Core::new(raw, memory).unwrap();
        unsafe {
            cpu.map_backing(0, 0x4000, 7, NonNull::new(backing.as_ptr()).unwrap())
                .unwrap()
        }
        cpu.write_register(arm::CPSR, &0xd3u64.to_le_bytes())
            .unwrap();
        cpu
    };
    let cpu = core(shared.processor());
    let _peer = core(shared.processor());
    for n in 1..=3u32 {
        // mov r0, #n
        cpu.write_memory(0, &(0xe3a00000 | n).to_le_bytes())
            .unwrap();
        run(&cpu, 0, 1);
        assert_eq!(value(&cpu, arm::CORE[0]), u64::from(n));
    }
}

#[test]
fn thumb_entries_preserve_itstate_and_interwork_at_every_budget_boundary() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0x1000, 0x1000, 7).unwrap();
    // movw r3,#0x1021; blx r3; mov r2,#3
    code(&cpu, &[0xe3013021, 0xe12fff33, 0xe3a02003]);
    // cmp r0,#0; ite eq; moveq r1,#1; movne.w r1,#2; bx lr
    let thumb = [0x2800u16, 0xbf0c, 0x2101, 0xf04f, 0x0102, 0x4770];
    cpu.write_memory(
        0x1020,
        &thumb
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    cpu.set_entry_trace(16).unwrap();

    let entries = [
        0x1000, 0x1004, 0x1020, 0x1022, 0x1024, 0x1026, 0x102a, 0x1008,
    ];
    for (r0, step) in [(0u32, 1), (1, 1), (0, 8), (1, 8)] {
        cpu.write_register(arm::CPSR, &0xd3u32.to_le_bytes())
            .unwrap();
        cpu.write_register(arm::CORE[0], &r0.to_le_bytes()).unwrap();
        let mut begin = entries[0];
        for _ in 0..entries.len() / step {
            assert!(matches!(run(&cpu, begin, step), Exit::Stopped));
            assert_eq!(cpu.instruction_progress(), Some(step as u64));
            begin = value(&cpu, Register::Pc) | ((value(&cpu, arm::CPSR) >> 5) & 1);
        }
        assert_eq!(cpu.take_entries().unwrap(), entries);
        assert_eq!(value(&cpu, arm::CORE[1]), u64::from(r0 + 1));
        assert_eq!(value(&cpu, arm::CORE[2]), 3);
        assert_eq!(begin, 0x100c);
    }
}

#[test]
fn thumb_framework_hints_respect_it_and_resume_in_thumb() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0x1000, 0x1000, 7).unwrap();
    // cmp r0,#0; it eq; wfieq; movs r1,#3
    cpu.write_memory(0x1000, &[0x00, 0x28, 0x08, 0xbf, 0x30, 0xbf, 0x03, 0x21])
        .unwrap();
    cpu.write_register(arm::CPSR, &0xd3u32.to_le_bytes())
        .unwrap();
    assert!(matches!(
        run(&cpu, 0x1001, 4),
        Exit::BeforeInstruction(0xe320_f003)
    ));
    assert_eq!(cpu.instruction_progress(), Some(2));
    assert_eq!(value(&cpu, Register::Pc), 0x1004);
    let cpsr = value(&cpu, arm::CPSR) as u32;
    armv7a::finish_thumb(&cpu, 0x1004, cpsr).unwrap();
    assert_eq!(value(&cpu, arm::CPSR) as u32, arm::thumb::advance_it(cpsr));
    run(&cpu, 0x1007, 1);
    assert_eq!(value(&cpu, arm::CORE[1]), 3);

    cpu.write_register(arm::CORE[0], &1u32.to_le_bytes())
        .unwrap();
    assert!(matches!(run(&cpu, 0x1001, 4), Exit::Stopped));
    assert_eq!(cpu.instruction_progress(), Some(4));
    assert_eq!(value(&cpu, Register::Pc), 0x1008);
}

#[test]
fn irq_from_thumb_restores_the_it_predicate_on_exception_return() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0, 0x2000, 7).unwrap();
    cpu.write_register(arm::CPSR, &0x53u32.to_le_bytes())
        .unwrap();
    // ARM IRQ handler: subs pc,lr,#4
    cpu.write_memory(0x18, &0xe25ef004u32.to_le_bytes())
        .unwrap();
    // cmp r0,#0; it eq; moveq r1,#1
    cpu.write_memory(0x1000, &[0x00, 0x28, 0x08, 0xbf, 0x01, 0x21])
        .unwrap();
    run(&cpu, 0x1001, 2);
    let interrupted = value(&cpu, arm::CPSR);
    assert_ne!(interrupted & 0x0600_fc00, 0);
    cpu.set_interrupt(Interrupt::Irq, true).unwrap();
    run(&cpu, 0x1005, 1);
    assert_eq!(cpu.instruction_progress(), Some(0));
    assert_eq!(value(&cpu, arm::SPSR), interrupted);
    assert_eq!(value(&cpu, Register::Pc), 0x18);
    cpu.set_interrupt(Interrupt::Irq, false).unwrap();
    run(&cpu, 0x18, 2);
    assert_eq!(cpu.instruction_progress(), Some(2));
    assert_eq!(value(&cpu, arm::CORE[1]), 1);
    assert_eq!(value(&cpu, Register::Pc), 0x1006);
}

#[test]
fn changing_the_second_thumb_halfword_invalidates_the_mmu_translation() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0, 0x200000, 7).unwrap();
    let descriptors = [0x0010_0c02u32, 0x0000_0c02];
    cpu.write_memory(
        0x4008,
        &descriptors.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>(),
    )
    .unwrap();
    let cp = |crn| arm::Coprocessor {
        cp: 15,
        opc1: 0,
        crn,
        crm: 0,
        opc2: 0,
        secure: 0,
        wide: false,
    };
    cpu.coprocessor(cp(2), Some(0x4000)).unwrap();
    cpu.coprocessor(cp(3), Some(0xffff_ffff)).unwrap();
    let sctlr = cpu.coprocessor(cp(1), None).unwrap();
    cpu.coprocessor(cp(1), Some(sctlr | 1)).unwrap();
    // mov.w r0,#1 at 0x2ffffe, crossing into a nonadjacent physical page.
    cpu.write_memory(0x1ffffe, &[0x4f, 0xf0]).unwrap();
    cpu.write_memory(0, &[0x01, 0x00]).unwrap();
    run(&cpu, 0x2fffff, 1);
    assert_eq!(value(&cpu, arm::CORE[0]), 1);
    cpu.write_memory(0, &[0x02, 0x00]).unwrap();
    run(&cpu, 0x2fffff, 1);
    assert_eq!(value(&cpu, arm::CORE[0]), 2);
}

#[test]
fn thumb_mmio_entry_is_counted_once() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0x1000, 0x1000, 7).unwrap();
    cpu.write_register(arm::CORE[1], &0x8000u32.to_le_bytes())
        .unwrap();
    // movs r2,#1; ldr r0,[r1]; adds r2,#1
    cpu.write_memory(0x1000, &[0x01, 0x22, 0x08, 0x68, 0x01, 0x32])
        .unwrap();
    let mut d = Device {
        cpu: &cpu,
        reads: vec![],
        writes: vec![],
        tighten: false,
    };
    unsafe {
        cpu.map_mmio(
            0x8000,
            0x1000,
            Mmio {
                read,
                write,
                data: (&mut d as *mut Device<'_>).cast(),
            },
        )
        .unwrap();
    }
    run(&cpu, 0x1001, 3);
    assert_eq!(cpu.instruction_progress(), Some(3));
    assert_eq!(d.reads, [2]);
    assert_eq!(value(&cpu, arm::CORE[0]), 42);
    assert_eq!(value(&cpu, arm::CORE[2]), 2);
}

#[test]
fn thumb_store_rewrites_a_previously_translated_instruction() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0x1000, 0x1000, 7).unwrap();
    // movs r0,#1; b 0x1020
    cpu.write_memory(0x1000, &[0x01, 0x20, 0x0d, 0xe0]).unwrap();
    // ldr r1,=0x2002; ldr r2,=0x1000; strh r1,[r2]; b 0x1000
    let words = [0x4901u16, 0x4a02, 0x8011, 0xe7eb, 0x2002, 0, 0x1000, 0];
    cpu.write_memory(
        0x1020,
        &words
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    run(&cpu, 0x1001, 7);
    assert_eq!(cpu.instruction_progress(), Some(7));
    assert_eq!(value(&cpu, arm::CORE[0]), 2);
}

#[test]
fn thumb_data_abort_enters_arm_with_the_architectural_fault_registers() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0, 0x2000, 7).unwrap();
    cpu.write_register(arm::CPSR, &0xd3u32.to_le_bytes())
        .unwrap();
    let cp = |crn, opc2| arm::Coprocessor {
        cp: 15,
        opc1: 0,
        crn,
        crm: 0,
        opc2,
        secure: 0,
        wide: false,
    };
    let sctlr = cpu.coprocessor(cp(1, 0), None).unwrap();
    cpu.coprocessor(cp(1, 0), Some(sctlr | 2)).unwrap();
    cpu.write_register(arm::CORE[1], &0x1001u32.to_le_bytes())
        .unwrap();
    // ldr r0,[r1] from an unaligned address
    cpu.write_memory(0x1000, &[0x08, 0x68]).unwrap();
    run(&cpu, 0x1001, 1);
    assert_eq!(cpu.instruction_progress(), Some(1));
    assert_eq!(value(&cpu, Register::Pc), 0x10);
    assert_eq!(value(&cpu, arm::SPSR), 0xf3);
    assert_eq!(value(&cpu, arm::CORE[14]), 0x1008);
    assert_eq!(cpu.coprocessor(cp(5, 0), None).unwrap(), 1);
    assert_eq!(cpu.coprocessor(cp(6, 0), None).unwrap(), 0x1001);
}

#[test]
fn ldr_pc_and_pop_pc_interwork_on_an_a7() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0x1000, 0x1000, 7).unwrap();
    // ldr pc,[pc,#-4]; .word 0x1021; mov r0,#7
    code(&cpu, &[0xe51ff004, 0x1021, 0xe3a00007]);
    cpu.write_memory(0x1020, &[0x00, 0xbd]).unwrap();
    cpu.write_register(arm::CORE[13], &0x1080u32.to_le_bytes())
        .unwrap();
    cpu.write_memory(0x1080, &0x1008u32.to_le_bytes()).unwrap();
    run(&cpu, 0x1000, 3);
    assert_eq!(cpu.instruction_progress(), Some(3));
    assert_eq!(value(&cpu, arm::CORE[0]), 7);
    assert_eq!(value(&cpu, arm::CPSR) & 0x20, 0);
}

#[test]
fn thumb_irq_unmask_is_a_boundary_before_the_following_entry() {
    let raw = crate::test_support::library()
        .open(Model::CortexA7, 1 << 20)
        .unwrap();
    let cpu = Core::new(raw, Memory::default()).unwrap();
    cpu.map_zeroed(0, 0x2000, 7).unwrap();
    cpu.write_register(arm::CPSR, &0xd3u32.to_le_bytes())
        .unwrap();
    // nop; cpsie i; movs r0,#7
    cpu.write_memory(0x1000, &[0x00, 0xbf, 0x62, 0xb6, 0x07, 0x20])
        .unwrap();
    cpu.set_interrupt(Interrupt::Irq, true).unwrap();
    run(&cpu, 0x1001, 4);
    assert_eq!(cpu.instruction_progress(), Some(2));
    assert_eq!(value(&cpu, Register::Pc), 0x18);
    assert_eq!(value(&cpu, arm::SPSR) & 0xa0, 0x20);
    assert_eq!(value(&cpu, arm::CORE[0]), 0);
}
