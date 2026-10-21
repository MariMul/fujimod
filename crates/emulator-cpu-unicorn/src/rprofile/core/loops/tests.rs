use super::super::tests::{get, processor_with_options, set, words};
use super::*;

fn execute(code: &[u32], mode: LoopMode, budget: usize, counter: u32) -> (Vec<u32>, u64, u64) {
    let cpu = processor_with_options(&Memory::default(), Options::default());
    cpu.map_zeroed(0, 0x4000, 7).unwrap();
    words(&cpu, 0x1000, code);
    set(&cpu, arm::CPSR, 0x13 | 0xa000_0000);
    set(&cpu, arm::CORE[0], counter);
    set(&cpu, arm::CORE[2], 0x2000);
    set(&cpu, arm::CORE[3], 200);
    cpu.set_loop_mode(mode).unwrap();
    cpu.set_loop_window(true).unwrap();
    cpu.run(Run {
        begin: 0x1000,
        until: None,
        watchdog_us: 0,
        instructions: Some(budget),
    })
    .unwrap();
    (
        arm::CORE
            .iter()
            .chain([arm::CPSR].iter())
            .map(|&r| get(&cpu, r))
            .collect(),
        cpu.instruction_progress().unwrap(),
        cpu.exit_counters().unwrap()["loop_accelerated_instructions"],
    )
}

#[test]
fn every_counted_loop_budget_matches_instruction_execution() {
    for code in [
        vec![0xe2500001, 0x1afffffd, 0xe320f003], // SUBS; BNE
        vec![0xe2800001, 0xe35000c8, 0x3afffffc, 0xe320f003],
        vec![0xe2800001, 0xe1500003, 0x3afffffc, 0xe320f003], // ADD; CMP; BLO
        vec![0xe2400001, 0xe3500009, 0x8afffffc, 0xe320f003], // SUB; CMP; BHI
    ] {
        for budget in (1..40).chain([100, 101, 199, 200, 201, 399, 600]) {
            let off = execute(&code, LoopMode::Disabled, budget, 100);
            let on = execute(&code, LoopMode::Accelerate, budget, 100);
            assert_eq!((&off.0, off.1), (&on.0, on.1), "{code:x?}, budget={budget}");
            if budget > 100 {
                assert!(on.2 > 0);
            }
        }
    }
}
#[test]
fn ram_poll_and_unconditional_fixed_point_match_at_every_instruction() {
    for code in [
        vec![0xe5921000, 0xe3110001, 0x0afffffc], // LDR; TST; BEQ
        vec![0xeafffffe],                         // B .
    ] {
        for budget in 1..40 {
            let off = execute(&code, LoopMode::Disabled, budget, 1);
            let on = execute(&code, LoopMode::Accelerate, budget, 1);
            assert_eq!((&off.0, off.1), (&on.0, on.1), "budget={budget}");
            if budget > 10 {
                assert!(on.2 > 0);
            }
        }
    }
}
#[test]
fn stores_system_instructions_and_conditional_bodies_never_prove() {
    for word in [
        0xe5821000, 0xee110f10, 0xe320f003, 0xe1921f9f, 0xe592f000, 0xe5b21004, 0x02800001,
        0xe8b20003, 0xe1d210b0,
    ] {
        assert!(decode(word).is_none(), "{word:08x}");
    }
    assert!(target(0x1004, 0xebfffffd).is_none());
}
#[test]
fn wrapping_counters_and_false_conditions_are_conservative() {
    for value in [0, 1, 2, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
        let code = [0xe2500001, 0x1afffffd, 0xe320f003];
        let off = execute(&code, LoopMode::Disabled, 101, value);
        let on = execute(&code, LoopMode::Accelerate, 101, value);
        assert_eq!((&off.0, off.1), (&on.0, on.1));
    }
}
#[test]
fn shifted_operands_and_flags_follow_the_cpu() {
    for code in [
        vec![0xe1b010a0, 0xeafffffd], // MOVS r1,r0,LSR #1; B
        vec![0xe2101102, 0xeafffffd], // ANDS immediate with rotated carry
    ] {
        for value in [0, 1, 2, 0x8000_0001, u32::MAX] {
            let off = execute(&code, LoopMode::Disabled, 100, value);
            let on = execute(&code, LoopMode::Accelerate, 100, value);
            assert_eq!((&off.0, off.1), (&on.0, on.1));
        }
    }
}

#[test]
fn stable_permission_expires_and_observers_keep_every_instruction() {
    let cpu = processor_with_options(&Memory::default(), Options::default());
    cpu.map_zeroed(0, 0x4000, 7).unwrap();
    words(&cpu, 0x1000, &[0xe2500001, 0x1afffffd, 0xe320f003]);
    set(&cpu, arm::CPSR, 0xd3);
    cpu.set_loop_mode(LoopMode::Accelerate).unwrap();
    for stable in [true, false] {
        set(&cpu, arm::CORE[0], 1000);
        if stable {
            cpu.set_loop_window(true).unwrap();
        }
        let before = cpu.exit_counters().unwrap()["loop_accelerated_instructions"];
        super::super::tests::run(&cpu, 0x1000, 1500).unwrap();
        let after = cpu.exit_counters().unwrap()["loop_accelerated_instructions"];
        assert_eq!(after > before, stable);
    }
    unsafe fn observe(_: u64, _: u32, data: *mut c_void) {
        unsafe {
            *data.cast::<usize>() += 1;
        }
    }
    let mut hits = 0usize;
    let hook = unsafe {
        cpu.install_hook(Hook::Instruction {
            callback: observe,
            data: (&mut hits as *mut usize).cast(),
            range: HookRange::Inclusive {
                begin: 0x1000,
                end: 0x1004,
            },
        })
        .unwrap()
    };
    set(&cpu, arm::CORE[0], 1000);
    cpu.set_loop_window(true).unwrap();
    let before = cpu.exit_counters().unwrap()["loop_accelerated_instructions"];
    super::super::tests::run(&cpu, 0x1000, 1500).unwrap();
    assert_eq!(hits, 1500);
    assert_eq!(
        cpu.exit_counters().unwrap()["loop_accelerated_instructions"],
        before
    );
    cpu.remove_hook(hook).unwrap();
}

#[test]
fn rewritten_code_and_ram_are_reproved_after_a_resume() {
    for mode in [LoopMode::Disabled, LoopMode::Measure, LoopMode::Accelerate] {
        let cpu = processor_with_options(&Memory::default(), Options::default());
        cpu.map_zeroed(0, 0x4000, 7).unwrap();
        words(
            &cpu,
            0x1000,
            &[0xe5921000, 0xe3110001, 0x0afffffc, 0xe320f003],
        );
        set(&cpu, arm::CPSR, 0xd3);
        set(&cpu, arm::CORE[2], 0x2000);
        cpu.set_loop_mode(mode).unwrap();
        cpu.set_loop_window(true).unwrap();
        super::super::tests::run(&cpu, 0x1000, 99).unwrap();
        words(&cpu, 0x2000, &[1]);
        cpu.set_loop_window(true).unwrap();
        super::super::tests::resume(&cpu, 100).unwrap();
        assert_eq!(cpu.instruction_progress(), Some(4));
        assert_eq!(get(&cpu, arm::CORE[1]), 1);
        assert_eq!(get(&cpu, arm::PC), 0x1010);
        // Replace the body with a store; it must execute once per iteration.
        words(&cpu, 0x1000, &[0xe5821000, 0xe2811001, 0xeafffffc]);
        cpu.set_loop_window(true).unwrap();
        super::super::tests::run(&cpu, 0x1000, 300).unwrap();
        assert_eq!(get(&cpu, arm::CORE[1]), 101);
        let mut value = [0; 4];
        cpu.read_memory(0x2000, &mut value).unwrap();
        assert_eq!(u32::from_le_bytes(value), 100);
    }
}

#[test]
fn measurement_executes_the_complete_counted_and_poll_loops() {
    for (code, counter) in [
        (vec![0xe2500001, 0x1afffffd, 0xe320f003], 100),
        (vec![0xe5921000, 0xe3110001, 0x0afffffc], 0),
    ] {
        for budget in [1, 2, 3, 4, 99, 199, 200, 201, 300] {
            let off = execute(&code, LoopMode::Disabled, budget, counter);
            let measured = execute(&code, LoopMode::Measure, budget, counter);
            assert_eq!(
                (&off.0, off.1),
                (&measured.0, measured.1),
                "code={code:x?} budget={budget}"
            );
            assert_eq!(measured.2, 0);
        }
    }
}

#[test]
fn address_stops_and_unbounded_runs_do_not_skip_loops() {
    for (until, instructions, expected) in [(Some(0x1004), Some(1000), 1), (None, None, 201)] {
        let cpu = processor_with_options(&Memory::default(), Options::default());
        cpu.map_zeroed(0, 0x4000, 7).unwrap();
        words(&cpu, 0x1000, &[0xe2500001, 0x1afffffd, 0xe320f003]);
        set(&cpu, arm::CPSR, 0xd3);
        set(&cpu, arm::CORE[0], 100);
        cpu.set_loop_mode(LoopMode::Accelerate).unwrap();
        cpu.set_loop_window(true).unwrap();
        cpu.run(Run {
            begin: 0x1000,
            until,
            watchdog_us: 0,
            instructions,
        })
        .unwrap();
        assert_eq!(cpu.instruction_progress(), Some(expected));
        assert_eq!(
            cpu.exit_counters().unwrap()["loop_accelerated_instructions"],
            0
        );
    }
}
