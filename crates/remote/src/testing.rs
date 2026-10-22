use super::*;
use crate::backend::Backend;
#[cfg(test)]
use crate::connection::Connection;
use serde_json::json;

pub fn facts() -> Facts {
    let hash = "00".repeat(32);
    let keys = [
        ("menu_ok_button", "MENU"),
        ("disp_back_button", "BACK"),
        ("selector.up", "UP"),
        ("selector.down", "DOWN"),
        ("selector.left", "LEFT"),
        ("selector.right", "RIGHT"),
        ("playback_button", "PLAY"),
        ("wifi_button", "FN"),
        ("shutter_button", "HALF"),
        ("shutter_button", "FULL"),
        ("position_dial", "AUTO"),
        ("command_control", "PREVIOUS"),
        ("command_control", "NEXT"),
    ];
    let button = |name: &str, key: &str| json!({"name":name,"location":"rear","manual_page":null,"face":null,"icon":null,"connection_ids":[],"assignable":false,"default_function":null,"role":null,"mechanical":false,"behavior":{"kind":"button","press":{"kind":"debug_key","name":key}}});
    let physical_controls = json!({
        "$schema":fujimod_catalog::controls::PHYSICAL_CONTROLS_SCHEMA_URI,"schema":1,"id":"synthetic-controls","body_id":"synthetic",
        "manual":{"url":"https://example.test/manual","title":"Synthetic controls","pages":[]},"analog_levels":1,
        "controls":{
            "menu_ok_button":button("MENU/OK button","MENU"),
            "playback_button":button("Playback button","PLAY"),
            "wifi_button":button("Function button","FN"),
            "shutter_button":{"name":"Shutter","location":"top","manual_page":null,"face":null,"icon":null,"connection_ids":[],"assignable":false,"default_function":null,"role":null,"mechanical":false,"behavior":{"kind":"shutter","stages":{"half_press":[{"kind":"debug_key","name":"HALF"}],"full_press":[{"kind":"debug_key","name":"HALF"},{"kind":"debug_key","name":"FULL"}]}}},
            "position_dial":{"name":"Position dial","location":"top","manual_page":null,"face":null,"icon":null,"connection_ids":[],"assignable":false,"default_function":null,"role":null,"mechanical":false,"behavior":{"kind":"dial","motion":{"kind":"positions","positions":[{"id":"A","label":"A","description":null,"key":{"kind":"debug_key","name":"AUTO"},"asserted":true}]},"press":null,"endless":false}},
            "command_control":{"name":"Command dial","location":"rear","manual_page":null,"face":null,"icon":null,"connection_ids":[],"assignable":false,"default_function":null,"role":null,"mechanical":false,"behavior":{"kind":"dial","motion":{"kind":"steps","decrease":{"kind":"debug_key","name":"PREVIOUS"},"increase":{"kind":"debug_key","name":"NEXT"}},"press":null,"endless":true}},
            "disp_back_button":button("DISP/BACK button","BACK"),
            "selector":{"name":"Selector","location":"rear","manual_page":null,"face":null,"icon":null,"connection_ids":[],"assignable":false,"default_function":null,"role":null,"mechanical":false,
                "behavior":{"kind":"pad","press":null,"directions":{"up":{"kind":"debug_key","name":"UP"},"down":{"kind":"debug_key","name":"DOWN"},"left":{"kind":"debug_key","name":"LEFT"},"right":{"kind":"debug_key","name":"RIGHT"}}}}
        }
    });
    let control_ids = json!({
        "$schema":fujimod_catalog::controls::CONTROL_IDS_SCHEMA_URI,"schema":1,"id":"synthetic-1-control-ids","firmware_id":"synthetic-1",
        "scanner":{"base":16,"count":1,"stride":4,"lookup":16,"sha256":hash},
        "input_readers":{"calibration":16,"scanner":{"stable_offset":0,"stable_width":1,"pressed_value":1,"released_value":0,"routine":{"address":16,"size":4,"sha256":hash}}},
        "native_key_names":{"rows":[],"reader":16,"size":4,"sha256":hash},
        "debug_keys":keys.iter().enumerate().map(|(n,(_,key))|json!({"name":key,"event":n})).collect::<Vec<_>>(),
        "inputs":keys.iter().enumerate().map(|(n,(id,key))|json!({"id":format!("input-{n}"),"event":n,"debug_key":key,"target":{"kind":"control","control_id":id},"scanner":null,"electrical":null})).collect::<Vec<_>>(),
        "interfaces":{}
    });
    serde_json::from_value(json!({
        "physical_controls":physical_controls,
        "control_ids":control_ids,
        "firmware":{
            "$schema":fujimod_catalog::remote::FIRMWARE_FACTS_SCHEMA_URI,"schema":1,"firmware_id":"synthetic-1","firmware_sha256":hash,"body_id":"synthetic",
            "uncached_alias":0,
            "identity_checks":[{"address":16,"size":4,"sha256":hash}],"configuration":{"kind":"direct","address":1024},
            "state":{"ui_mode":768,"system_mode":772,"menu_active":776,"pending_mode":780,"menu_node":784,"rec_mode":3,"setup_mode":5,"readiness":{"category_one":800,"category_two":808,"input_pending":816,"excluded_mask":0,"operation_alert_timer":820,"override_address":824}},
            "menus":{"roots_address":256,"root_count":2,"roles":{"shooting":0},"root_titles":{},"root_routes":{"0":{"entry":"shooting_menu","tab":null}},"page_tabs":{},"navigation":{"shooting_first_tab":0,"shooting_last_tab":0,"setup_first_tab":1},
                "layout":{"size":32,"fields":{"menu_id":0,"type_code":4,"setting_id":8,"rows_per_page":12,"flags_word":16,"parent_pointer":20,"item_array":24,"handlers_pointer":28},"row_size":8,"row_key":0,"row_child":4,"caption_words":2,"sentinel":4294967295u32,"max_kind":8,"nowrap_flags":0},
                "cursor_address":512,"cursor":{"size":32,"fields":{"pointer":0,"cursor":8,"subcursor":12,"visible_count":16,"value_cursor":20,"raw_count":24,"filtered_count":28},"status_offset":32,"status_stride":4,"status_width":4},"q_grid":{"rows":1,"columns":1,"menu_code":1}},
            "scanout":{"address":32768,"width":1,"height":1,"pixel_format":"bgra8888","stride_bytes":4,"instance":0,"layer":0,"channel":0,"pointer_address":832,"address_mask":4294967295u32},
            "resources":{"glyphs":{"registry_address":24576,"record_size":8,"icon_index":0,"data_address_offset":0,"data_size_offset":4},"language":{"kind":"direct","address":24576},"text_record_size":8,"text_variants_offset":2,"text_metrics_offset":4,"text_pointer_offset":0,"text_unit_bytes":2,"text_max_units":64,"private_text_first":57600,"private_text_last":57855,"private_text_bias":57600,"glyph_first":61440,"glyph_last":63743,"supplementary_address":25088,"supplementary_first":16,"last_caption":16,"supplementary_stride":66,"supplementary_max_units":28,
                "bitmaps":{"table":24576,"stride":8,"final_table":24576,"final_first_index":0,"last_index":0,"last_record":24576,"layout":{"data_bytes":4,"data_offset":0,"height_bytes":2,"height_offset":4,"kind_bytes":1,"kind_offset":6,"width_bytes":2,"width_offset":8}}}
        },"menu_manifest":null
    })).unwrap()
}
pub struct Synthetic {
    bytes: Vec<u8>,
    now: u64,
    sequence: u64,
    edges: Vec<Input>,
    waits: Vec<ReadSet>,
    value: u32,
    stuck: bool,
    reorder_on_down: bool,
}
impl Synthetic {
    pub fn new() -> Self {
        let mut s = Self {
            bytes: vec![0; 65536],
            now: 0,
            sequence: 0,
            edges: vec![],
            waits: vec![],
            value: 0,
            stuck: false,
            reorder_on_down: false,
        };
        for (address, value) in [(256, 4096), (260, 4352), (768, 3), (824, 1), (832, 32768)] {
            s.word(address, value);
        }
        s.node(4096, 0, 0, 8192, 12288);
        s.node(4352, 1, 4096, 8448, 12544);
        for (a, words) in [
            (8192, vec![10, 0, 20, 4352, u32::MAX, 0]),
            (8448, vec![0, 0, 1, 0, u32::MAX, 0]),
            (12288, vec![28672, 0, 28688, 0]),
            (12544, vec![28704, 0, 28720, 0]),
        ] {
            for (n, v) in words.iter().enumerate() {
                s.word(a + n * 4, *v);
            }
        }
        for (a, label) in [
            (28672, "FIRST"),
            (28688, "SETTING"),
            (28704, "OFF"),
            (28720, "ON"),
        ] {
            s.bytes[a..a + label.len()].copy_from_slice(label.as_bytes());
        }
        s
    }
    fn word(&mut self, a: usize, v: u32) {
        self.bytes[a..a + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn get(&self, a: usize) -> u32 {
        u32::from_le_bytes(self.bytes[a..a + 4].try_into().unwrap())
    }
    fn node(&mut self, a: usize, code: u32, parent: u32, rows: u32, captions: u32) {
        for (n, v) in [code, 1, 0, 7, 0, parent, rows, captions]
            .iter()
            .enumerate()
        {
            self.word(a + n * 4, *v);
        }
    }
    fn active(&mut self, node: u32, cursor: u32) {
        self.word(776, 1);
        self.word(784, node);
        self.word(512, node);
        self.word(520, cursor);
        self.word(532, 0);
    }
}
impl Backend for Synthetic {
    fn now(&self) -> u64 {
        self.now
    }
    fn memory<'a>(
        &'a mut self,
        a: u64,
        n: usize,
        reads: &'a mut ReadSet,
    ) -> LocalFuture<'a, Vec<u8>> {
        Box::pin(async move {
            reads.stores.push((a, a + n as u64));
            self.bytes
                .get(a as usize..a as usize + n)
                .map(|b| b.to_vec())
                .ok_or("Unmapped synthetic memory".into())
        })
    }
    fn input<'a>(&'a mut self, input: &'a Input, _: &'a Facts) -> LocalFuture<'a, ()> {
        Box::pin(async move {
            self.edges.push(input.clone());
            if input.state != InputState::Pressed {
                return Ok(());
            }
            match input.control.id() {
                "menu_ok_button" if self.get(776) == 0 => self.active(4096, 0),
                "menu_ok_button" => {
                    self.value = self.get(520);
                    self.active(4096, 1);
                }
                "selector.right" => self.active(4352, self.value),
                "selector.down" => {
                    self.word(520, 1);
                    if self.reorder_on_down {
                        self.bytes[8192..8208].rotate_left(8);
                        self.reorder_on_down = false;
                    }
                }
                "selector.up" => self.word(520, 0),
                "disp_back_button" => self.word(776, 0),
                _ => {}
            }
            Ok(())
        })
    }
    fn accepted(&mut self, _: &Input, _: &mut ReadSet) -> Result<bool> {
        Ok(true)
    }
    fn frame<'a>(&'a mut self, _: &'a Facts, reads: &'a mut ReadSet) -> LocalFuture<'a, Capture> {
        Box::pin(async move {
            reads.devices.insert(7);
            Ok(Capture {
                frame: fujimod_recording::Frame::rgb(1, 1, vec![1, 2, 3])?,
                time: self.now,
                clock: Clock::GuestTicks,
                output: "lcd".into(),
                sequence: self.sequence,
                reads: reads.clone(),
                completion_reads: reads.clone(),
            })
        })
    }
    fn wait<'a>(&'a mut self, reads: &'a ReadSet, deadline: u64) -> LocalFuture<'a, ()> {
        Box::pin(async move {
            self.waits.push(reads.clone());
            self.now = (self.now + 1).min(deadline);
            if !self.stuck {
                self.sequence += 1;
            }
            futures_lite::future::yield_now().await;
            Ok(())
        })
    }
    fn control_position(&mut self, _: &str) -> Result<Option<String>> {
        Ok(None)
    }
}
#[test]
fn navigate_select_reopen_and_capture() {
    futures_lite::future::block_on(async {
        let mut r = Connection::connect(Synthetic::new(), facts()).unwrap();
        let item = menu("shooting").with_key(20);
        let v = r.navigate(&item, 100).await.unwrap();
        assert!(v.menu().unwrap().selected);
        r.select(&option(item.clone(), "ON"), 100).await.unwrap();
        r.navigate(&item, 100).await.unwrap();
        operations::press(&mut r, &control("selector.right"), 100)
            .await
            .unwrap();
        let v = r.read(&option(item, "ON").into()).await.unwrap();
        assert!(v.menu().unwrap().selected);
        assert_eq!(v.menu().unwrap().value.as_deref(), Some("ON"));
        let frame = r.screenshot(100).await.unwrap();
        assert_eq!(frame.time, 1);
        assert_eq!(r.backend.waits.last().unwrap().devices, [7].into());
    });
}
#[test]
fn option_value_uses_selected_row_instead_of_viewport_origin() {
    futures_lite::future::block_on(async {
        let mut backend = Synthetic::new();
        backend.active(4352, 1);
        let mut remote = Connection::connect(backend, facts()).unwrap();
        let item = menu("shooting").with_key(20);
        for (label, selected) in [("ON", true), ("OFF", false)] {
            let snapshot = remote
                .read(&option(item.clone(), label).into())
                .await
                .unwrap();
            let value = snapshot.menu().unwrap();
            assert_eq!(value.selected, selected);
            assert_eq!(value.value.as_deref(), Some("ON"));
        }
        assert_eq!(
            remote
                .read(&item.into())
                .await
                .unwrap()
                .menu()
                .unwrap()
                .value,
            None
        );
    });
}

#[test]
fn resolution_is_live_and_strict() {
    futures_lite::future::block_on(async {
        let mut r = Connection::connect(Synthetic::new(), facts()).unwrap();
        let reference = menu("shooting").with_key(20).into();
        assert_eq!(
            r.read(&reference).await.unwrap().menu().unwrap().label,
            "SETTING"
        );
        r.backend.bytes[28688..28695].copy_from_slice(b"CHANGED");
        assert_eq!(
            r.read(&reference).await.unwrap().menu().unwrap().label,
            "CHANGED"
        );
        r.backend.word(8192, 20);
        assert!(
            r.read(&reference)
                .await
                .unwrap_err()
                .contains("matched 2 rows")
        );
    });
}
#[test]
fn frame_timeout_names_condition() {
    futures_lite::future::block_on(async {
        let mut backend = Synthetic::new();
        backend.stuck = true;
        let mut r = Connection::connect(backend, facts()).unwrap();
        assert!(
            r.screenshot(3)
                .await
                .err()
                .unwrap()
                .contains("next completed lcd frame")
        );
    });
}
#[test]
fn predicate_tracks_only_its_reads() {
    futures_lite::future::block_on(async {
        let mut r = Connection::connect(Synthetic::new(), facts()).unwrap();
        operations::wait_until(
            &mut r,
            |view| {
                Box::pin(async move {
                    let _ = view.read(&control("selector.down").into()).await?;
                    Ok(view.now() > 0)
                })
            },
            3,
            "clock progress",
        )
        .await
        .unwrap();
        assert!(!r.backend.waits[0].stores.is_empty());
        assert!(r.backend.waits[0].devices.is_empty());
    });
}

#[test]
fn navigation_resolves_again_after_each_edge() {
    futures_lite::future::block_on(async {
        let mut backend = Synthetic::new();
        backend.reorder_on_down = true;
        let mut remote = Connection::connect(backend, facts()).unwrap();
        let value = remote
            .navigate(&menu("shooting").with_key(20), 100)
            .await
            .unwrap();
        assert!(value.menu().unwrap().selected);
        assert_eq!(value.menu().unwrap().index, Some(0));
        assert!(
            remote
                .backend
                .edges
                .iter()
                .any(|i| i.control.id() == "selector.up")
        );
    });
}
#[test]
fn predicate_timeout_reports_last_snapshot() {
    futures_lite::future::block_on(async {
        let mut remote = Connection::connect(Synthetic::new(), facts()).unwrap();
        let error = operations::wait_until(
            &mut remote,
            |view| {
                Box::pin(async move {
                    view.read(&control("selector.down").into()).await?;
                    Ok(false)
                })
            },
            2,
            "test condition",
        )
        .await
        .unwrap_err();
        assert!(error.contains("test condition"));
        assert!(error.contains("last snapshots"));
        assert!(error.contains("ControlSnapshot"));
    });
}

#[test]
fn input_task_boundary_is_live_and_tracks_tables_and_receive_states() {
    use fujimod_catalog::remote::{InputTasks, ModeBusy};
    futures_lite::future::block_on(async {
        let mut facts = facts();
        facts.input_tasks = Some(InputTasks {
            headers: vec![900, 908],
            entries: vec![0x10, 0x20],
            address_mask: 0x3fffffff,
            address_limit: 65536,
            max_count: 128,
            stride: 0x38,
            descriptor_offset: 8,
            entry_offset: 8,
            descriptor_size: 0x28,
            state_offset: 12,
            receive_state: 0x12,
            busy: vec![ModeBusy {
                mode: 3,
                address: 920,
                width: 8,
            }],
        });
        let mut backend = Synthetic::new();
        for (address, value) in [
            (900, 35000),
            (904, 2),
            (908, 35500),
            (912, 1),
            (35008, 36000),
            (35064, 36040),
            (35508, 36100),
            (36008, 0x10),
            (36048, 0x20),
            (36108, 0x30),
            (35012, 0x12),
            (35068, 0x12),
        ] {
            backend.word(address, value);
        }
        let mut r = Connection::connect(backend, facts).unwrap();
        let reference = control("selector.down").into();
        let ready = r.read(&reference).await.unwrap();
        assert!(ready.state.ready);
        assert!(
            ready
                .reads
                .stores
                .iter()
                .any(|&(lo, hi)| lo <= 35012 && hi > 35012)
        );
        r.backend.word(35068, 0);
        assert!(!r.read(&reference).await.unwrap().state.ready);
        r.backend.word(35068, 0x12);
        r.backend.word(920, 1);
        assert!(!r.read(&reference).await.unwrap().state.ready);
        r.backend.word(920, 0);
        r.backend.word(36108, 0x10);
        assert!(r.read(&reference).await.unwrap_err().contains("ambiguous"));
        r.backend.word(36108, 0x30);
        r.backend.word(36048, 0x40);
        assert!(r.read(&reference).await.unwrap_err().contains("missing"));
        r.backend.word(904, 129);
        assert!(
            r.read(&reference)
                .await
                .unwrap_err()
                .contains("unavailable")
        );
    });
}
