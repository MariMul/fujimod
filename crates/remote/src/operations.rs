//! Actions composed once over the four primitives; callers own assertions.
use crate::*;
pub async fn press<R: Remote + ?Sized>(r: &mut R, c: &Control, deadline: u64) -> Result<()> {
    r.set_input(
        &Input {
            control: c.clone(),
            state: InputState::Pressed,
        },
        deadline,
    )
    .await?;
    r.set_input(
        &Input {
            control: c.clone(),
            state: InputState::Released,
        },
        deadline,
    )
    .await
}
pub async fn turn<R: Remote + ?Sized>(
    r: &mut R,
    c: &Control,
    steps: i32,
    deadline: u64,
) -> Result<()> {
    if steps == 0 {
        return Ok(());
    }
    r.set_input(
        &Input {
            control: c.clone(),
            state: InputState::Turn(steps),
        },
        deadline,
    )
    .await
}
struct Tracked<'a, R: ?Sized> {
    remote: &'a mut R,
    reads: ReadSet,
    last: Vec<Snapshot>,
}
impl<R: Observation + ?Sized> Observation for Tracked<'_, R> {
    fn now(&self) -> u64 {
        self.remote.now()
    }
    fn read<'a>(&'a mut self, reference: &'a Reference) -> LocalFuture<'a, Snapshot> {
        Box::pin(async move {
            let v = self.remote.read(reference).await?;
            self.reads.extend(v.reads.clone());
            self.last.push(v.clone());
            Ok(v)
        })
    }
    fn frame(&mut self) -> LocalFuture<'_, Capture> {
        Box::pin(async move {
            let v = self.remote.frame().await?;
            self.reads.extend(v.reads.clone());
            Ok(v)
        })
    }
}
pub async fn wait_until<R: Remote + ?Sized>(
    r: &mut R,
    mut predicate: impl for<'a> FnMut(&'a mut dyn Observation) -> LocalFuture<'a, bool>,
    deadline: u64,
    condition: &str,
) -> Result<()> {
    loop {
        let mut view = Tracked {
            remote: r,
            reads: ReadSet::default(),
            last: vec![],
        };
        if predicate(&mut view).await? {
            return Ok(());
        }
        let reads = view.reads;
        let last = view.last;
        if r.now() >= deadline {
            return Err(format!(
                "Timed out waiting for {condition}; last snapshots: {last:?}; read set: {reads:?}"
            ));
        }
        r.wait(&reads, deadline).await?;
    }
}
async fn step<R: Remote + ?Sized>(
    r: &mut R,
    reference: &Reference,
    role: &str,
    deadline: u64,
) -> Result<Snapshot> {
    let before = r.read(reference).await?;
    press(r, &control(role), deadline).await?;
    loop {
        let value = r.read(reference).await?;
        if value.state.ready && value.state != before.state {
            return Ok(value);
        }
        if r.now() >= deadline {
            return Err(format!(
                "Timed out waiting for {role} navigation; last state: {:?}",
                value.state
            ));
        }
        r.wait(&value.reads, deadline).await?;
    }
}
async fn root<R: Remote + ?Sized>(
    r: &mut R,
    facts: &Facts,
    name: &str,
    deadline: u64,
) -> Result<()> {
    let reference: Reference = menu(name).into();
    let initial = r.read(&reference).await?;
    if initial.state.menu_active && initial.state.node == initial.menu()?.node {
        return Ok(());
    }
    let code = facts
        .firmware
        .menus
        .roles
        .get(name)
        .copied()
        .or_else(|| name.parse().ok())
        .ok_or("Unknown root role")?;
    let route = facts
        .firmware
        .menus
        .root_routes
        .get(&code)
        .ok_or_else(|| format!("No located entry route for menu {name}"))?;
    for _ in 0..64 {
        if !r.read(&reference).await?.state.menu_active {
            break;
        }
        step(r, &reference, "back", deadline).await?;
    }
    if r.read(&reference).await?.state.menu_active {
        return Err(format!("Could not close menus while entering {name}"));
    }
    step(r, &reference, "menu", deadline).await?;
    match route.entry.as_str() {
        "shooting_menu" => {}
        "setup_menu" => {
            step(r, &reference, "left", deadline).await?;
            let tab = route.tab.ok_or("Setup entry route lacks a tab key")?;
            for _ in 0..64 {
                let value = r.read(&reference).await?;
                if value.state.current_key == Some(tab) {
                    break;
                }
                let current = value
                    .state
                    .current_key
                    .ok_or("Tab rail has no selected row")?;
                step(
                    r,
                    &reference,
                    if current < tab { "down" } else { "up" },
                    deadline,
                )
                .await?;
            }
            if r.read(&reference).await?.state.current_key != Some(tab) {
                return Err(format!("Could not reach tab {tab}"));
            }
            step(r, &reference, "right", deadline).await?;
        }
        other => return Err(format!("Unsupported root entry strategy {other}")),
    }
    let value = r.read(&reference).await?;
    if !value.state.menu_active || value.state.node != value.menu()?.node {
        return Err(format!("Entry route {name} reached {:?}", value.state));
    }
    Ok(())
}
async fn focus<R: Remote + ?Sized>(
    r: &mut R,
    reference: &Reference,
    deadline: u64,
) -> Result<Snapshot> {
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..1024 {
        let value = r.read(reference).await?;
        let target = value.menu()?;
        if !value.state.menu_active || value.state.node != target.node {
            return Err(format!(
                "Navigation left menu {reference:?}; {:?}",
                value.state
            ));
        }
        if !value.state.ready {
            if r.now() >= deadline {
                return Err(format!(
                    "Timed out waiting for navigation readiness: {:?}",
                    value.state
                ));
            }
            r.wait(&value.reads, deadline).await?;
            continue;
        }
        if target.selected {
            return Ok(value);
        }
        if !target.enabled {
            return Err(format!("Menu {reference:?} is disabled"));
        }
        if !seen.insert(value.state.cursor) {
            return Err(format!(
                "Menu {reference:?} is unavailable; repeated cursor {}",
                value.state.cursor
            ));
        }
        let index = target.index.ok_or("Page has no focusable row")?;
        step(
            r,
            reference,
            if value.state.cursor < index {
                "down"
            } else {
                "up"
            },
            deadline,
        )
        .await?;
    }
    Err(format!("Navigation exceeded row bound for {reference:?}"))
}
pub async fn navigate<R: Remote + ?Sized>(
    r: &mut R,
    facts: &Facts,
    item: &Menu,
    deadline: u64,
) -> Result<Snapshot> {
    let reference = Reference::Menu(item.clone());
    let initial = r.read(&reference).await?;
    if initial.state.menu_active && initial.state.node == initial.menu()?.node {
        return focus(r, &reference, deadline).await;
    }
    root(r, facts, &initial.menu()?.root, deadline).await?;
    for depth in 0..64 {
        let value = r.read(&reference).await?;
        if value.state.menu_active && value.state.node == value.menu()?.node {
            return focus(r, &reference, deadline).await;
        }
        let ancestor = value
            .menu()?
            .ancestors
            .get(depth)
            .ok_or_else(|| format!("Menu {item:?} has no opening row at depth {depth}"))?
            .clone();
        focus(r, &Reference::Menu(ancestor.clone()), deadline).await?;
        step(r, &Reference::Menu(ancestor), "right", deadline).await?;
    }
    Err(format!("Navigation exceeded ancestor bound for {item:?}"))
}
pub async fn select<R: Remote + ?Sized>(
    r: &mut R,
    facts: &Facts,
    choice: &OptionReference,
    deadline: u64,
) -> Result<()> {
    let reference = Reference::Option(choice.clone());
    let target = r.read(&reference).await?;
    if target.state.node != target.menu()?.node || !target.state.menu_active {
        navigate(r, facts, choice.item(), deadline).await?;
        let parent = r.read(&Reference::Menu(choice.item().clone())).await?;
        if parent.menu()?.key.is_some() {
            step(r, &reference, "right", deadline).await?;
        }
    }
    focus(r, &reference, deadline).await?;
    press(r, &control("ok"), deadline).await
}
pub async fn screenshot<R: Remote + ?Sized>(r: &mut R, deadline: u64) -> Result<Capture> {
    let before = r.frame().await?;
    loop {
        let frame = r.frame().await?;
        if frame.sequence != before.sequence {
            return Ok(frame);
        }
        if r.now() >= deadline {
            return Err(format!(
                "Timed out waiting for next completed {} frame; last sequence={}, time={}",
                frame.output, frame.sequence, frame.time
            ));
        }
        r.wait(&frame.completion_reads, deadline).await?;
    }
}
