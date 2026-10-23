//! Native calls to the same C ABI exercised by the Node integration checks.
use fujimod_emulator_javascript::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn error(handle: u32) -> String {
    let mut bytes = Bytes::default();
    assert_eq!(unsafe { camera_error(handle, &mut bytes) }, 0);
    String::from_utf8(
        unsafe { std::slice::from_raw_parts(bytes.data, bytes.len as usize) }.to_vec(),
    )
    .unwrap()
}
fn command(
    handle: u32,
    action: &str,
    options: Value,
    upload: &[u8],
) -> Result<(Value, Vec<u8>), String> {
    let request = serde_json::to_vec(&json!({"action":action,"options":options})).unwrap();
    let mut out = ResultView::default();
    if unsafe {
        camera_command(
            handle,
            request.as_ptr(),
            request.len() as u32,
            upload.as_ptr(),
            upload.len() as u32,
            &mut out,
        )
    } != 0
    {
        return Err(error(handle));
    }
    let value = serde_json::from_slice(unsafe {
        std::slice::from_raw_parts(out.json.data, out.json.len as usize)
    })
    .unwrap();
    let bytes =
        unsafe { std::slice::from_raw_parts(out.bytes.data, out.bytes.len as usize) }.to_vec();
    Ok((value, bytes))
}
fn create(scanner: bool) -> u32 {
    create_with(scanner, |_| {})
}
fn create_with(scanner: bool, configure: impl FnOnce(&mut Value)) -> u32 {
    // The optional guest program continuously samples ADC1 and writes one
    // stable byte, standing in for a stock scanner in this synthetic fixture.
    // Host inspection never writes its acknowledgment.
    let words: &[u32] = if scanner {
        &[
            0xe30a0000, 0xe34f0ff8, // r0 = ADC base
            0xe3a01008, 0xe5801004, // continuous input mode
            0xe3a01002, 0xe5801008, // select channel 1
            0xe3081005, 0xe5801000, // START, continuous read-through mode
            0xe5902034, 0xe3520c01, // ldr result[1]; cmp sample, #256
            0xe3a02000, 0x33a02001, // stable = sample < 256
            0xe3003101, 0xe3403001, 0xe5c32000, 0xeafffff7,
        ]
    } else {
        &[0xeafffffe]
    };
    let firmware: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mapping = json!({"address":0x10000,"physical":0x10000,"size":0x4000,"permissions":3});
    let mut config = json!({"schema":1,"firmwareId":"generated","bootSource":"internal","initialPositions":{},"graph":{
        "schema":1,"firmwareId":"generated","bootSource":"internal","quantum":17,"translationCacheBytes":1048576,
        "execution":{"processors":[{"model":"cortex-r5f","released":true,"registers":[["Pc",0x10000]],"mprofile":null,"rprofile":{"cp14_identity":0x4003,"tcm":{"atcm":{"bytes":0,"reset":0},"btcm":{"bytes":0,"reset":0}}}}],
            "ram":[{"base":0x10000,"size":0x4000}],"mappings":[{"core":0,"address":0x10000,"physical":0x10000,"size":0x4000,"permissions":7}],"dma_mappings":[mapping],"decoders":[],"instructions_per_tick":[24,1],"peripherals":[{"core":0,"address":0xfffb3000u64,"physical":0xfffb3000u64,"size":4096}]},
        "devices":{"schema":1,"boot_entry":"loader","wires":{},"nodes":[{"name":"irq","boot_entries":["loader"],"windows":[{"base":0xfffb3000u64,"size":4096}],"dependencies":[],"model":{"kind":"exr2-interrupt-controller","options":{"arbitration":"unique"},"core":0,"base":0xfffb3000u64,"routes":null,"software":[]}}]},
        "firmware":{"size":firmware.len(),"sha256":fujimod_firmware::sha256(&firmware),"format":"raw","mappings":[{"name":"loop","offset":0,"size":firmware.len(),"base":0x10000,"encoding":"raw","executable":true}]}
    }});
    config["graph"]["execution"]["peripherals"]
        .as_array_mut()
        .unwrap()
        .push(json!({"core":0,"address":0xfff8a000u64,"physical":0xfff8a000u64,"size":4096}));
    config["graph"]["devices"]["nodes"].as_array_mut().unwrap().push(json!({"name":"adc","boot_entries":["loader"],"windows":[{"base":0xfff8a000u64,"size":4096}],"dependencies":[],"model":{"kind":"adc","options":{"conversion_ticks":4,"samples":[0,0,0,0,0,0,0,0,0,0,0,0]}}}));
    config["graph"]["controls"] = json!({"controls":[{"id":"lever","label":"Lever","kind":"selector","positions":["off","on"],"initial":"off","unavailable":null}],"circuits":[{"controls":["lever"],"cases":[{"positions":["off"],"signals":[{"kind":"analog","node":"adc","channel":0,"value":10}]},{"positions":["on"],"signals":[{"kind":"analog","node":"adc","channel":0,"value":900}]}]}]});
    if scanner {
        config["graph"]["controls"] = json!({"controls":[{"id":"button","label":"Button","kind":"button","positions":["released","pressed"],"initial":"released","unavailable":null}],"circuits":[{"controls":["button"],"cases":[{"positions":["released"],"signals":[{"kind":"analog","node":"adc","channel":1,"value":1023}]},{"positions":["pressed"],"signals":[{"kind":"analog","node":"adc","channel":1,"value":100}]}]}]});
        config["graph"]["contactDriver"] = json!({"poll_ticks":10,"max_phase_ticks":1000,
            "catalog":[{"id":"button","label":"Button","kind":"button","supported":true}],
            "actuators":[{"id":"button","momentary":true,"states":[
                {"position":"released","ready":[{"address":0x10100,"bytes":[0]}],"expected":[{"address":0x10101,"bytes":[0]}]},
                {"position":"pressed","ready":[{"address":0x10100,"bytes":[0]}],"expected":[{"address":0x10101,"bytes":[1]}]}]}]});
    }
    let format = json!({"sectors":0x800000,"max_dirty_sectors":131072,"partition_start":2048,"sectors_per_cluster":64,"label":"TEST"});
    config["graph"]["execution"]["peripherals"]
        .as_array_mut()
        .unwrap()
        .push(json!({"core":0,"address":0xfff70000u64,"physical":0xfff70000u64,"size":4096}));
    config["graph"]["devices"]["nodes"].as_array_mut().unwrap().push(json!({"name":"sd","boot_entries":["loader"],"windows":[{"base":0xfff70000u64,"size":0x108}],"dependencies":[],"model":{"kind":"sd-host","options":{"protocol":"sdhc","transfer_ticks":2,"clock_ticks":2,"rca":4660,"ram_bytes":0x14000},"card":{"kind":"formatted","options":format,"files":"sd_files"},"present":true,"protected":false,"irq":{"Core":{"core":0,"line":80}}}}));
    config["graph"]["card"] = json!({"node":"sd","format":format});
    config["graph"]["scene"]["files"] = json!({"sd_files":{"KEEP.TXT":"file"}});
    config["graph"]["assets"] = json!({"file":{"kind":"bytes","bytes":[1,2,3]}});
    configure(&mut config);
    let config = serde_json::to_vec(&config).unwrap();
    let handle = unsafe {
        camera_session_create(
            config.as_ptr(),
            config.len() as u32,
            firmware.as_ptr(),
            firmware.len() as u32,
        )
    };
    assert_ne!(handle, 0, "{}", error(0));
    handle
}

fn load_snapshot(bytes: &[u8]) -> u32 {
    let handle = unsafe { camera_snapshot_create(bytes.as_ptr(), bytes.len() as u32) };
    assert_ne!(handle, 0, "{}", error(0));
    handle
}

#[test]
fn portable_archive_reconstructs_a_destroyed_session_and_continues_exactly() {
    let source = create(false);
    command(source, "watch_pcs", json!({"addresses":[0x10000]}), &[]).unwrap();
    command(
        source,
        "set_position",
        json!({"id":"lever","position":"on"}),
        &[],
    )
    .unwrap();
    assert_eq!(camera_run_until(source, 11), 0, "{}", error(source));
    let before = command(source, "fingerprint", json!({}), &[]).unwrap().0;
    let (metadata, bytes) = command(source, "export_snapshot", json!({}), &[]).unwrap();
    assert_eq!(metadata["tick"], "11");
    assert_eq!(metadata["snapshot"]["length"], bytes.len());
    assert_eq!(
        command(source, "fingerprint", json!({}), &[]).unwrap().0,
        before
    );
    assert_eq!(camera_run_until(source, 53), 0, "{}", error(source));
    let expected = command(source, "fingerprint", json!({}), &[]).unwrap().0;
    assert_eq!(camera_destroy(source), 0);
    let loaded = load_snapshot(&bytes);
    assert_eq!(
        command(loaded, "fingerprint", json!({}), &[]).unwrap().0,
        before
    );
    assert_eq!(
        command(loaded, "state", json!({}), &[]).unwrap().0["positions"]["lever"],
        "on"
    );
    assert_eq!(camera_run_until(loaded, 53), 0, "{}", error(loaded));
    assert_eq!(
        command(loaded, "fingerprint", json!({}), &[]).unwrap().0,
        expected
    );
    assert_eq!(camera_destroy(loaded), 0);
    let mut corrupt = bytes;
    corrupt.truncate(corrupt.len() - 1);
    assert_eq!(
        unsafe { camera_snapshot_create(corrupt.as_ptr(), corrupt.len() as u32) },
        0
    );
}

#[test]
fn error_report_exports_a_healthy_retained_point_without_changing_the_failed_session() {
    let source = create_with(false, |c| {
        c["graph"]["execution"]["processors"][0]["registers"] = json!([["Pc", 0xfffd0000u32]]);
    });
    command(source, "checkpoint", json!({}), &[]).unwrap();
    assert_ne!(camera_run_until(source, 1), 0);
    let original_error = error(source);
    let failed = command(source, "fingerprint", json!({}), &[]).unwrap().0;
    let (report, bytes) = command(source, "error_report", json!({}), &[]).unwrap();
    assert!(!bytes.is_empty());
    assert_eq!(report["report"]["snapshot"]["tick"], "0");
    assert_ne!(report["report"]["snapshot"]["source"], "current");
    assert_eq!(
        command(source, "fingerprint", json!({}), &[]).unwrap().0,
        failed
    );
    assert_eq!(camera_destroy(source), 0);
    let loaded = load_snapshot(&bytes);
    let mut status = Status::default();
    assert_eq!(unsafe { camera_status(loaded, &mut status) }, 0);
    assert_eq!(status.failed, 0);
    assert_eq!(status.ticks, 0);
    assert_ne!(camera_run_until(loaded, 1), 0);
    assert_eq!(error(loaded), original_error);
    assert_eq!(camera_destroy(loaded), 0);
}

#[test]
fn powered_off_archive_preserves_power_controls_and_its_cold_reset_image() {
    let source = create(false);
    command(
        source,
        "set_position",
        json!({"id":"lever","position":"on"}),
        &[],
    )
    .unwrap();
    command(source, "power", json!({"on":false}), &[]).unwrap();
    let (_, bytes) = command(source, "export_snapshot", json!({}), &[]).unwrap();
    assert_eq!(camera_destroy(source), 0);
    let loaded = load_snapshot(&bytes);
    let state = command(loaded, "state", json!({}), &[]).unwrap().0;
    assert_eq!(state["poweredOn"], false);
    assert_eq!(state["positions"]["lever"], "on");
    assert_eq!(camera_run_until(loaded, 5), 0);
    let mut status = Status::default();
    assert_eq!(unsafe { camera_status(loaded, &mut status) }, 0);
    assert_eq!(status.ticks, 0);
    command(loaded, "power", json!({"on":true}), &[]).unwrap();
    assert_eq!(camera_run_until(loaded, 5), 0, "{}", error(loaded));
    assert_eq!(unsafe { camera_status(loaded, &mut status) }, 0);
    assert_eq!(status.ticks, 5);
    assert_eq!(camera_destroy(loaded), 0);
}

#[test]
fn cbor_history_preserves_u64_runs_and_restores_failed_replay_at_ratio_24() {
    #[derive(Deserialize, Serialize)]
    struct Log {
        schema: u32,
        owner: u32,
        generation: u64,
        entries: Vec<fujimod_emulator::session::history::Entry>,
    }
    let h = create(false);
    let cold = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    assert_eq!(cold["schema"], 1);
    assert_eq!(cold["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, cold);
    let checkpoint = command(h, "checkpoint", json!({}), &[]).unwrap().0;
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, cold);
    assert_eq!(checkpoint["tick"], "0");
    assert_eq!(camera_run_until(h, 5), 0, "{}", error(h));
    let at_five = command(h, "error_report", json!({}), &[]).unwrap().0["report"].clone();
    let five_fingerprint = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    assert_ne!(five_fingerprint, cold);
    let (summary, bytes) = command(h, "history", json!({}), &[]).unwrap();
    assert_eq!(summary["entries"], 1);
    let mut log: Log = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(log.owner, h);
    assert!(matches!(
        log.entries[0].input,
        fujimod_emulator::session::history::Input::Run {
            instructions: u64::MAX,
            ..
        }
    ));
    assert_eq!(
        command(h, "undo", json!({}), &[]).unwrap().0,
        json!({"tick":"0","instructions":["0"]})
    );
    let before = command(h, "error_report", json!({}), &[]).unwrap().0["report"].clone();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, cold);
    log.entries[0].after.instructions[0] += 1;
    let mut bad = Vec::new();
    ciborium::into_writer(&log, &mut bad).unwrap();
    assert!(
        command(h, "replay", json!({}), &bad)
            .unwrap_err()
            .contains("differs")
    );
    assert_eq!(
        command(h, "error_report", json!({}), &[]).unwrap().0["report"].clone(),
        before
    );
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, cold);
    assert_eq!(
        command(h, "replay", json!({}), &bytes).unwrap().0,
        json!({"tick":"5","instructions":["120"]})
    );
    assert_eq!(
        command(h, "error_report", json!({}), &[]).unwrap().0["report"].clone(),
        at_five
    );
    assert_eq!(
        command(h, "fingerprint", json!({}), &[]).unwrap().0,
        five_fingerprint
    );
    command(h, "restore", json!({"id":checkpoint["id"]}), &[]).unwrap();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, cold);
    assert_eq!(camera_run_until(h, 9), 0, "{}", error(h));
    assert_eq!(
        command(h, "rollback", json!({"tick":"5"}), &[]).unwrap().0,
        json!({"tick":"5","instructions":["120"]})
    );
    assert_eq!(
        command(h, "error_report", json!({}), &[]).unwrap().0["report"].clone(),
        at_five
    );
    assert_eq!(
        command(h, "fingerprint", json!({}), &[]).unwrap().0,
        five_fingerprint
    );
    let other = create(false);
    command(other, "checkpoint", json!({}), &[]).unwrap();
    assert!(
        command(other, "restore", json!({"id":checkpoint["id"]}), &[])
            .unwrap_err()
            .contains("checkpoint ID")
    );
    assert!(
        command(other, "replay", json!({}), &bytes)
            .unwrap_err()
            .contains("another session")
    );
    assert_eq!(camera_destroy(other), 0);
    assert_eq!(camera_destroy(h), 0);
}

#[test]
fn late_pc_witnesses_are_observational_and_capture_only_future_entries() {
    let h = create(false);
    assert_eq!(camera_run_until(h, 5), 0, "{}", error(h));
    let before = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    let history_before = command(h, "history", json!({}), &[]).unwrap();
    command(h, "watch_pcs", json!({"addresses":[0x10000]}), &[]).unwrap();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, before);
    assert_eq!(
        command(h, "history", json!({}), &[]).unwrap(),
        history_before
    );
    assert_eq!(
        command(h, "profile", json!({}), &[]).unwrap().0["pcEntries"],
        json!([])
    );
    assert_eq!(camera_run_until(h, 6), 0, "{}", error(h));
    let entries = command(h, "profile", json!({}), &[]).unwrap().0["pcEntries"].clone();
    assert_eq!(entries.as_array().unwrap().len(), 1);
    assert!(entries[0]["tick"].as_u64().unwrap() >= 5);
    assert_eq!(entries[0]["pc"], 0x10000);
    assert_eq!(camera_run_until(h, 7), 0, "{}", error(h));
    assert_eq!(
        command(h, "profile", json!({}), &[]).unwrap().0["pcEntries"],
        entries
    );
    assert!(command(h, "watch_pcs", json!({"addresses":[0x10000]}), &[]).is_err());
    assert_eq!(camera_destroy(h), 0);
}

#[test]
fn positions_drive_external_levels_and_restore_with_the_machine() {
    let h = create(false);
    let initial = command(h, "checkpoint", json!({}), &[]).unwrap().0;
    let before = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    let read = command(h, "memory_read", json!({"address":"65536","size":4}), &[])
        .unwrap()
        .1;
    assert_eq!(read, [0xfe, 0xff, 0xff, 0xea]);
    assert!(
        command(
            h,
            "memory_read",
            json!({"address":0xfff8a000u64.to_string(),"size":4}),
            &[]
        )
        .is_err()
    );
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, before);
    for (position, value) in [("on", 900), ("off", 10)] {
        let state = command(
            h,
            "set_position",
            json!({"id":"lever","position":position}),
            &[],
        )
        .unwrap()
        .0;
        assert_eq!(state["positions"]["lever"], position);
        let report = command(h, "error_report", json!({}), &[]).unwrap().0;
        let adc = report["report"]["bus"]["devices"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["kind"] == "exr2-adc")
            .unwrap();
        assert_eq!(adc["state"]["external"][0], value);
    }
    let after = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    let log = command(h, "history", json!({}), &[]).unwrap().1;
    command(h, "restore", json!({"id":initial["id"]}), &[]).unwrap();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, before);
    command(h, "replay", json!({}), &log).unwrap();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, after);
    assert_eq!(camera_destroy(h), 0);
}

#[test]
fn queued_physical_taps_wait_for_guest_acknowledgments_and_restore_in_flight() {
    let h = create(true);
    command(h, "checkpoint", json!({}), &[]).unwrap();
    for _ in 0..2 {
        command(h, "press", json!({"id":"button"}), &[]).unwrap();
        command(h, "release", json!({"id":"button"}), &[]).unwrap();
    }
    let before = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    for _ in 0..10 {
        let state = command(h, "state", json!({}), &[]).unwrap().0;
        assert_eq!(state["positions"]["button"], "pressed");
        assert_eq!(state["inputQueue"]["pending"], 3);
        assert_eq!(state["inputs"][0]["supported"], true);
    }
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, before);
    assert!(
        command(
            h,
            "set_position",
            json!({"id":"button","position":"released"}),
            &[]
        )
        .is_err()
    );
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, before);
    let saved = command(h, "checkpoint", json!({}), &[]).unwrap().0;
    assert_eq!(camera_run_until(h, 100), 0, "{}", error(h));
    let after = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    let state = command(h, "state", json!({}), &[]).unwrap().0;
    assert_eq!(state["inputQueue"]["completed"], 4);
    assert_eq!(state["inputQueue"]["active"], Value::Null);
    assert_eq!(state["positions"]["button"], "released");
    command(h, "restore", json!({"id":saved["id"]}), &[]).unwrap();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, before);
    assert_eq!(camera_run_until(h, 100), 0, "{}", error(h));
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, after);
    assert_eq!(camera_destroy(h), 0);
}

#[test]
fn full_contact_queue_reserves_release_and_refuses_without_losing_accepted_taps() {
    let h = create(true);
    for _ in 0..32 {
        command(h, "press", json!({"id":"button"}), &[]).unwrap();
        command(h, "release", json!({"id":"button"}), &[]).unwrap();
    }
    let before = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    assert!(
        command(h, "press", json!({"id":"button"}), &[])
            .unwrap_err()
            .contains("queue is full")
    );
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, before);
    assert_eq!(camera_run_until(h, 900), 0, "{}", error(h));
    let state = command(h, "state", json!({}), &[]).unwrap().0;
    assert_eq!(state["inputQueue"]["completed"], 64);
    assert_eq!(state["positions"]["button"], "released");
    assert_eq!(camera_destroy(h), 0);
}

#[test]
fn durable_storage_cold_lifecycle_and_held_boot_contacts_use_the_same_c_abi() {
    let h = create(true);
    let initial = command(h, "checkpoint", json!({}), &[]).unwrap().0;
    let saved = command(h, "save", json!({}), &[]).unwrap();
    assert_eq!(saved.0["format"], "camera-media-cbor");
    assert!(!saved.1.is_empty());
    command(
        h,
        "restart_card",
        json!({"files":[{"path":"NEW.BIN","offset":0,"length":4}]}),
        &[9, 8, 7, 6],
    )
    .unwrap();
    assert_eq!(
        command(h, "card_read", json!({"path":"KEEP.TXT"}), &[])
            .unwrap()
            .1,
        vec![1, 2, 3]
    );
    assert_eq!(
        command(h, "card_read", json!({"path":"NEW.BIN"}), &[])
            .unwrap()
            .1,
        vec![9, 8, 7, 6]
    );
    assert!(command(h, "restore", json!({"id":initial["id"]}), &[]).is_err());
    command(h, "power_cycle", json!({"inputs":[{"id":"button"}]}), &[]).unwrap();
    let state = command(h, "state", json!({}), &[]).unwrap().0;
    assert_eq!(state["positions"]["button"], "pressed");
    assert_eq!(state["inputQueue"]["pending"], 0);
    let mut status = Status::default();
    assert_eq!(unsafe { camera_status(h, &mut status) }, 0);
    assert_eq!((status.ticks, status.instructions), (0, 0));
    assert_eq!(camera_run_until(h, 200), 0);
    assert_eq!(
        command(h, "memory_read", json!({"address":"65793","size":1}), &[])
            .unwrap()
            .1,
        vec![1]
    );
    command(h, "release", json!({"id":"button"}), &[]).unwrap();
    assert_eq!(camera_run_until(h, 400), 0);
    assert_eq!(
        command(h, "memory_read", json!({"address":"65793","size":1}), &[])
            .unwrap()
            .1,
        vec![0]
    );
    command(h, "power", json!({"on":false}), &[]).unwrap();
    assert_eq!(
        command(h, "state", json!({}), &[]).unwrap().0["poweredOn"],
        false
    );
    assert_eq!(camera_run_until(h, 9999), 0);
    assert_eq!(unsafe { camera_status(h, &mut status) }, 0);
    assert_eq!(status.instructions, 0);
    assert_eq!(
        command(h, "memory_read", json!({"address":"65793","size":1}), &[])
            .unwrap()
            .1,
        vec![0]
    );
    let stable = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    assert!(command(h, "power_cycle", json!({"inputs":[{"id":"missing"}]}), &[]).is_err());
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, stable);
    let off = command(h, "save", json!({}), &[]).unwrap().1;
    let other = create(true);
    command(other, "restore_durable", json!({}), &off).unwrap();
    assert_eq!(
        command(other, "state", json!({}), &[]).unwrap().0["poweredOn"],
        false
    );
    assert_eq!(
        command(other, "card_read", json!({"path":"NEW.BIN"}), &[])
            .unwrap()
            .1,
        vec![9, 8, 7, 6]
    );
    command(other, "power", json!({"on":true}), &[]).unwrap();
    assert_eq!(
        command(other, "state", json!({}), &[]).unwrap().0["poweredOn"],
        true
    );
    let before = command(other, "fingerprint", json!({}), &[]).unwrap().0;
    let mut corrupt = off.clone();
    corrupt.push(0);
    assert!(command(other, "restore_durable", json!({}), &corrupt).is_err());
    assert_eq!(
        command(other, "fingerprint", json!({}), &[]).unwrap().0,
        before
    );
    command(other, "restore_durable", json!({}), &saved.1).unwrap();
    assert!(command(other, "card_read", json!({"path":"NEW.BIN"}), &[]).is_err());
    assert_eq!(camera_destroy(h), 0);
    assert_eq!(camera_destroy(other), 0);
}

#[test]
fn rotary_admission_is_atomic_and_in_flight_phases_replay_through_the_command_api() {
    let h = create_with(false, |config| {
        let graph = &mut config["graph"];
        graph["execution"]["peripherals"]
            .as_array_mut()
            .unwrap()
            .push(json!({"core":0,"address":0xfff89000u64,"physical":0xfff89000u64,"size":4096}));
        let nodes = graph["devices"]["nodes"].as_array_mut().unwrap();
        nodes.push(json!({"name":"pads","boot_entries":["loader"],"windows":[{"base":0xfff89000u64,"size":1152}],"dependencies":[],"model":{"kind":"gpio","options":{"input_levels":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"output_levels":[]},"irqs":[]}}));
        nodes.push(json!({"name":"encoder","boot_entries":["loader"],"windows":[],"dependencies":["pads"],"model":{"kind":"encoder","gpio":"pads","counter":null,"options":{"pins":[{"port":3,"pin":6},{"port":3,"pin":7}],"phase_ticks":24,"rest_phase":0}}}));
        graph["contactDriver"] = json!({"poll_ticks":10,"max_phase_ticks":1000,
            "catalog":[{"id":"lever","label":"Lever","kind":"selector","supported":true},{"id":"ring","label":"Ring","kind":"rotary","supported":true,"relative":true}],
            "actuators":[{"id":"lever","momentary":false,"states":[{"position":"off","ready":[],"expected":[]},{"position":"on","ready":[],"expected":[]}]}],
            "interfaces":[{"kind":"rotary","id":"ring","node":"encoder","cycles_per_step":-1}]});
    });
    command(h, "checkpoint", json!({}), &[]).unwrap();
    let cold = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    let entries = command(h, "history", json!({}), &[]).unwrap().0;
    assert!(
        command(h, "rotate", json!({"id":"ring","steps":385}), &[])
            .unwrap_err()
            .contains("bound")
    );
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, cold);
    assert_eq!(command(h, "history", json!({}), &[]).unwrap().0, entries);
    command(h, "rotate", json!({"id":"ring","steps":2}), &[]).unwrap();
    let queued = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    assert_eq!(camera_run_until(h, 100), 0, "{}", error(h));
    let middle = command(h, "fingerprint", json!({}), &[]).unwrap().0;
    assert_ne!(middle, cold);
    let (_, log) = command(h, "history", json!({}), &[]).unwrap();
    command(h, "undo", json!({}), &[]).unwrap();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, queued);
    command(h, "undo", json!({}), &[]).unwrap();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, cold);
    command(h, "replay", json!({}), &log).unwrap();
    assert_eq!(command(h, "fingerprint", json!({}), &[]).unwrap().0, middle);
    assert_eq!(camera_destroy(h), 0);
}

#[test]
fn raw_usb_commands_preserve_opaque_packets_and_never_run_the_guest() {
    let handle =
        create_with(false, |config| {
            let graph = &mut config["graph"];
            graph["devices"]["wires"]["cable"] = json!({"logic-level":true});
            graph["devices"]["nodes"].as_array_mut().unwrap().push(json!({
            "name":"usb", "boot_entries":["loader"],
            "windows":[{"base":0xfff54000u64,"size":4096},{"base":0xfff55000u64,"size":4096}],
            "dependencies":["irq"], "model":{"kind":"usb", "cable":"cable",
                "options":{"endpoints":{"transfer_ticks":4,"packet_bytes":64,"dma_limit":1024,
                    "out_endpoints":[[0,0],[1,1]],"in_endpoints":[[128,0],[130,1],[131,2]]}},
                "endpoints":{"irq":{"Core":{"core":0,"line":84}},
                    "dma_irq":{"Core":{"core":0,"line":85}},"ram_bytes":0x14000}}
        }));
            for base in [0xfff54000u64, 0xfff55000] {
                graph["execution"]["peripherals"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({
                        "core":0,"address":base,"physical":base,"size":4096
                    }));
            }
        });
    let status = || {
        command(handle, "usb_status", json!({"node":"usb"}), &[])
            .unwrap()
            .0
    };
    let before = status();
    assert_eq!(before, status());
    assert_eq!(before["connected"], true);
    command(handle, "usb_reset", json!({"node":"usb"}), &[]).unwrap();
    command(
        handle,
        "usb_setup",
        json!({"node":"usb"}),
        &[0x80, 6, 0, 1, 0, 0, 18, 0],
    )
    .unwrap();
    command(
        handle,
        "usb_out",
        json!({"node":"usb","endpoint":1}),
        &[0xde, 0xad, 0xbe, 0xef],
    )
    .unwrap();
    assert_eq!(status()["endpoint"]["setup_length"], 8);
    assert_eq!(status()["endpoint"]["rx_length"][1], 4);
    assert!(
        command(
            handle,
            "usb_in",
            json!({"node":"usb","endpoint":130,"size":64}),
            &[]
        )
        .unwrap_err()
        .contains("no packet")
    );
    assert!(command(handle, "usb_out", json!({"node":"usb","endpoint":1}), &[1]).is_err());
    assert!(command(handle, "usb_status", json!({"node":"usb"}), &[1]).is_err());
    assert!(command(handle, "usb_setup", json!({"node":"usb"}), &[1]).is_err());
    let mut state = Status::default();
    assert_eq!(unsafe { camera_status(handle, &mut state) }, 0);
    assert_eq!(state.ticks, 0);
    assert_eq!(state.instructions, 0);
    assert_eq!(state.failed, 0);
    assert_eq!(camera_destroy(handle), 0);
}

#[test]
fn device_workers_are_host_preferences_without_history_or_fingerprint_effects() {
    let handle = create(false);
    assert_eq!(
        command(handle, "device_workers", json!({}), &[]).unwrap().0,
        json!({"count":0,"effective":0})
    );
    let before = command(handle, "fingerprint", json!({}), &[]).unwrap();
    assert_eq!(
        command(handle, "device_workers", json!({"count":2}), &[])
            .unwrap()
            .0,
        json!({"count":2,"effective":0})
    );
    assert!(command(handle, "device_workers", json!({"count":1}), &[]).is_err());
    assert_eq!(
        command(handle, "device_workers", json!({"count":0}), &[])
            .unwrap()
            .0,
        json!({"count":0,"effective":0})
    );
    assert_eq!(
        command(handle, "fingerprint", json!({}), &[]).unwrap(),
        before
    );
    assert_eq!(camera_destroy(handle), 0);
}

#[test]
fn interactive_device_workers_default_inline_and_allow_explicit_stopped_selection() {
    let policy = b"interactive";
    assert_eq!(
        unsafe { camera_policy_setup(policy.as_ptr(), policy.len() as u32) },
        0
    );
    let handle = create(false);
    assert_eq!(
        command(handle, "device_workers", json!({}), &[]).unwrap().0,
        json!({"count":0,"effective":0})
    );
    let before = command(handle, "fingerprint", json!({}), &[]).unwrap();
    assert_eq!(
        command(handle, "device_workers", json!({"count":2}), &[])
            .unwrap()
            .0,
        json!({"count":2,"effective":2})
    );
    assert_eq!(
        command(handle, "fingerprint", json!({}), &[]).unwrap(),
        before
    );
    assert_eq!(camera_run_until(handle, 5), 0, "{}", error(handle));
    assert_eq!(
        command(handle, "device_workers", json!({}), &[]).unwrap().0,
        json!({"count":2,"effective":2})
    );
    assert_eq!(
        command(handle, "device_workers", json!({"count":0}), &[])
            .unwrap()
            .0,
        json!({"count":0,"effective":0})
    );
    assert_eq!(camera_destroy(handle), 0);
    let policy = b"exact";
    assert_eq!(
        unsafe { camera_policy_setup(policy.as_ptr(), policy.len() as u32) },
        0
    );
}
