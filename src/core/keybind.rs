use evdev::{Device, EventType, InputEventKind, Key as EvKey};
use futures::channel::oneshot;
use futures::FutureExt;
use futures::{channel::mpsc, select, StreamExt};
use gpui_kit::Context;
use std::collections::HashSet;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::app::SoundboardApp;

type Recorder = Arc<Mutex<Option<oneshot::Sender<String>>>>;

#[derive(Clone)]
pub struct GlobalShortcutManager {
    tx: mpsc::UnboundedSender<Command>,
    recorder: Recorder,
}

enum Command {
    Reload,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum ModKey {
    Ctrl,
    Alt,
    Shift,
    Super,
}

#[derive(Clone)]
struct ParsedShortcut {
    sound_id: String,
    modifiers: HashSet<ModKey>,
    keys: Vec<EvKey>,
}

impl GlobalShortcutManager {
    pub fn start(
        app: gpui_kit::WeakEntity<SoundboardApp>,
        cx: &mut Context<SoundboardApp>,
    ) -> Self {
        let (tx, mut commands) = mpsc::unbounded();

        let recorder: Recorder = Arc::new(Mutex::new(None));
        let recorder_for_task = recorder.clone();

        cx.spawn(async move |_this, cx| loop {
            let sounds = match app.update(cx, |app, _cx| {
                app.library
                    .sounds
                    .iter()
                    .filter_map(|sound| {
                        sound
                            .keybind
                            .as_ref()
                            .map(|keybind| (sound.id.clone(), sound.name.clone(), keybind.clone()))
                    })
                    .collect::<Vec<_>>()
            }) {
                Ok(sounds) => sounds,

                Err(err) => {
                    eprintln!("Soundboard entity no longer exists: {err}");
                    return;
                }
            };

            println!("Registering {} global shortcut(s) via evdev", sounds.len());

            let mut shortcuts = Vec::new();
            for (id, name, keybind) in &sounds {
                match parse_keybind(keybind) {
                    Some((modifiers, keys)) => {
                        println!("  {name} -> {keybind} ({id})");
                        shortcuts.push(ParsedShortcut {
                            sound_id: id.clone(),
                            modifiers,
                            keys,
                        });
                    }
                    None => {
                        eprintln!(
                            "  Skipping '{name}': couldn't map keybind '{keybind}' to a key."
                        );
                    }
                }
            }

            if shortcuts.is_empty() {
                println!("No global sound keybinds configured.");
            }

            let stop_flag = Arc::new(AtomicBool::new(false));
            let (event_tx, mut event_rx) = mpsc::unbounded::<String>();

            let thread_stop = stop_flag.clone();
            let rec = recorder_for_task.clone();
            let worker = thread::spawn(move || {
                if let Err(err) = run_evdev_loop(shortcuts, thread_stop, event_tx, rec) {
                    eprintln!("soundboard: global shortcut listener stopped: {err}");
                }
            });

            loop {
                select! {
                    sound_id = event_rx.next().fuse() => {
                        let Some(sound_id) = sound_id else { break; };

                        let _ = app.update(cx, |app, cx| {
                            app.play_sound(sound_id, cx);
                        });
                    }

                    command = commands.next().fuse() => {
                        match command {
                            Some(Command::Reload) => {
                                println!("Reloading global shortcuts...");
                                break;
                            }

                            None => {
                                println!("Global shortcut manager stopped.");
                                stop_flag.store(true, Ordering::SeqCst);
                                let _ = worker.join();
                                return;
                            }
                        }
                    }
                }
            }

            stop_flag.store(true, Ordering::SeqCst);
            let _ = worker.join();
        })
        .detach();

        Self { tx, recorder }
    }

    pub fn reload(&self) {
        let _ = self.tx.unbounded_send(Command::Reload);
    }

    pub fn record(&self) -> oneshot::Receiver<String> {
        let (tx, rx) = oneshot::channel();
        *self.recorder.lock().unwrap() = Some(tx);
        rx
    }

    pub fn _cancel_recording(&self) {
        self.recorder.lock().unwrap().take();
    }
}

/// Runs on a dedicated OS thread. Polls every readable keyboard device
/// directly via evdev (`/dev/input/event*`), which is why this works
/// identically under what ever wm or de you throw at it no
/// portal support and no per-compositor config needed. The user's
/// needs to be in the `input`.
/// hopefully we get a better way than this oh boy
fn run_evdev_loop(
    shortcuts: Vec<ParsedShortcut>,
    stop: Arc<AtomicBool>,
    tx: mpsc::UnboundedSender<String>,
    recorder: Recorder,
) -> std::io::Result<()> {
    let mut devices = open_keyboard_devices();

    if devices.is_empty() {
        eprintln!(
            "soundboard: no readable keyboard devices found under /dev/input/event*. \
             run `sudo usermod -aG input $USER` and log back in for global hotkeys to work.\
             This is required to make the keybinds work even if the window is not focused.
             "
        );
    }

    let mut held_mods: HashSet<ModKey> = HashSet::new();

    while !stop.load(Ordering::SeqCst) {
        let mut saw_events = false;

        for device in &mut devices {
            loop {
                match device.fetch_events() {
                    Ok(events) => {
                        saw_events = true;

                        for ev in events {
                            if ev.event_type() != EventType::KEY {
                                continue;
                            }
                            let InputEventKind::Key(key) = ev.kind() else {
                                continue;
                            };

                            match ev.value() {
                                1 => {
                                    // key down
                                    if let Some(m) = modifier_for_key(key) {
                                        held_mods.insert(m);
                                    } else {
                                        // if the UI is waiting for a keybind, capture this
                                        // key instead of triggering sounds.
                                        {
                                            let mut slot = recorder.lock().unwrap();
                                            if slot.is_some() {
                                                if let Some(name) = name_for_key(key) {
                                                    let sender = slot.take().unwrap();
                                                    let _ =
                                                        sender.send(format_combo(&held_mods, name));
                                                }
                                                continue;
                                            }
                                        }

                                        for shortcut in &shortcuts {
                                            if shortcut.keys.contains(&key)
                                                && shortcut.modifiers == held_mods
                                            {
                                                let _ =
                                                    tx.unbounded_send(shortcut.sound_id.clone());
                                            }
                                        }
                                    }
                                }
                                0 => {
                                    // key up
                                    if let Some(m) = modifier_for_key(key) {
                                        held_mods.remove(&m);
                                    }
                                }
                                _ => {} // 2 = autorepeat, ignored
                            }
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(err) => {
                        eprintln!("soundboard: error reading input device: {err}");
                        break;
                    }
                }
            }
        }

        if !saw_events {
            thread::sleep(Duration::from_millis(15));
        }
    }

    Ok(())
}

fn open_keyboard_devices() -> Vec<Device> {
    let mut devices = Vec::new();

    for (path, device) in evdev::enumerate() {
        let looks_like_keyboard = device
            .supported_keys()
            .map(|keys| keys.contains(EvKey::KEY_A) && keys.contains(EvKey::KEY_ENTER))
            .unwrap_or(false);

        if !looks_like_keyboard {
            continue;
        }

        // Make the fd non-blocking so our polling loop never stalls on
        // one idle device while others have pending events.
        let fd = device.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL, 0);
            if flags >= 0 {
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        println!(
            "soundboard: listening on keyboard device {}",
            path.display()
        );
        devices.push(device);
    }

    devices
}

fn modifier_for_key(key: EvKey) -> Option<ModKey> {
    match key {
        EvKey::KEY_LEFTCTRL | EvKey::KEY_RIGHTCTRL => Some(ModKey::Ctrl),
        EvKey::KEY_LEFTALT | EvKey::KEY_RIGHTALT => Some(ModKey::Alt),
        EvKey::KEY_LEFTSHIFT | EvKey::KEY_RIGHTSHIFT => Some(ModKey::Shift),
        EvKey::KEY_LEFTMETA | EvKey::KEY_RIGHTMETA => Some(ModKey::Super),
        _ => None,
    }
}

fn parse_keybind(combo: &str) -> Option<(HashSet<ModKey>, Vec<EvKey>)> {
    let combo = combo.to_lowercase();
    let mut parts: Vec<&str> = combo.split('-').collect();
    let key_str = parts.pop()?;

    let mut modifiers = HashSet::new();
    for part in parts {
        modifiers.insert(modifier_from_str(part)?);
    }

    let keys = key_candidates_from_str(key_str)?;
    Some((modifiers, keys))
}

fn modifier_from_str(s: &str) -> Option<ModKey> {
    match s {
        "ctrl" | "control" => Some(ModKey::Ctrl),
        "alt" | "option" => Some(ModKey::Alt),
        "shift" => Some(ModKey::Shift),
        "cmd" | "super" | "meta" | "win" | "windows" => Some(ModKey::Super),
        _ => None,
    }
}

/// Canonical key names used when recording. Each one maps to exactly one
/// physical key, so number row and numpad keys never share a name.
const NAMES: &[&str] = &[
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z", //
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9", //
    "kp0",
    "kp1",
    "kp2",
    "kp3",
    "kp4",
    "kp5",
    "kp6",
    "kp7",
    "kp8",
    "kp9", //
    "kpenter",
    "kpminus",
    "kpplus",
    "kpasterisk",
    "kpslash",
    "kpdot",
    "kpequal", //
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12", //
    "space",
    "tab",
    "enter",
    "escape",
    "backspace",
    "delete",
    "insert",
    "home",
    "end",
    "pageup",
    "pagedown",
    "up",
    "down",
    "left",
    "right",
    "capslock", //
    "minus",
    "equal",
    "comma",
    "period",
    "slash",
    "semicolon",
    "apostrophe",
    "leftbracket",
    "rightbracket",
    "backslash",
    "grave",
];

/// Reverse lookup: physical key -> canonical name (for recording).
fn name_for_key(key: EvKey) -> Option<&'static str> {
    NAMES
        .iter()
        .copied()
        .find(|n| key_candidates_from_str(n).map_or(false, |k| k.contains(&key)))
}

/// Builds a keybind string like "ctrl-shift-kp1" from held modifiers + key name.
fn format_combo(mods: &HashSet<ModKey>, key: &str) -> String {
    let mut parts = Vec::new();
    if mods.contains(&ModKey::Ctrl) {
        parts.push("ctrl");
    }
    if mods.contains(&ModKey::Alt) {
        parts.push("alt");
    }
    if mods.contains(&ModKey::Shift) {
        parts.push("shift");
    }
    if mods.contains(&ModKey::Super) {
        parts.push("super");
    }
    parts.push(key);
    parts.join("-")
}

fn key_candidates_from_str(s: &str) -> Option<Vec<EvKey>> {
    Some(match s {
        "a" => vec![EvKey::KEY_A],
        "b" => vec![EvKey::KEY_B],
        "c" => vec![EvKey::KEY_C],
        "d" => vec![EvKey::KEY_D],
        "e" => vec![EvKey::KEY_E],
        "f" => vec![EvKey::KEY_F],
        "g" => vec![EvKey::KEY_G],
        "h" => vec![EvKey::KEY_H],
        "i" => vec![EvKey::KEY_I],
        "j" => vec![EvKey::KEY_J],
        "k" => vec![EvKey::KEY_K],
        "l" => vec![EvKey::KEY_L],
        "m" => vec![EvKey::KEY_M],
        "n" => vec![EvKey::KEY_N],
        "o" => vec![EvKey::KEY_O],
        "p" => vec![EvKey::KEY_P],
        "q" => vec![EvKey::KEY_Q],
        "r" => vec![EvKey::KEY_R],
        "s" => vec![EvKey::KEY_S],
        "t" => vec![EvKey::KEY_T],
        "u" => vec![EvKey::KEY_U],
        "v" => vec![EvKey::KEY_V],
        "w" => vec![EvKey::KEY_W],
        "x" => vec![EvKey::KEY_X],
        "y" => vec![EvKey::KEY_Y],
        "z" => vec![EvKey::KEY_Z],

        // number row keys (row only)
        "0" => vec![EvKey::KEY_0],
        "1" => vec![EvKey::KEY_1],
        "2" => vec![EvKey::KEY_2],
        "3" => vec![EvKey::KEY_3],
        "4" => vec![EvKey::KEY_4],
        "5" => vec![EvKey::KEY_5],
        "6" => vec![EvKey::KEY_6],
        "7" => vec![EvKey::KEY_7],
        "8" => vec![EvKey::KEY_8],
        "9" => vec![EvKey::KEY_9],

        // numpad keys (numpad only)
        "kp0" => vec![EvKey::KEY_KP0],
        "kp1" => vec![EvKey::KEY_KP1],
        "kp2" => vec![EvKey::KEY_KP2],
        "kp3" => vec![EvKey::KEY_KP3],
        "kp4" => vec![EvKey::KEY_KP4],
        "kp5" => vec![EvKey::KEY_KP5],
        "kp6" => vec![EvKey::KEY_KP6],
        "kp7" => vec![EvKey::KEY_KP7],
        "kp8" => vec![EvKey::KEY_KP8],
        "kp9" => vec![EvKey::KEY_KP9],
        "kpenter" => vec![EvKey::KEY_KPENTER],
        "kpminus" => vec![EvKey::KEY_KPMINUS],
        "kpplus" | "plus" | "+" => vec![EvKey::KEY_KPPLUS],
        "kpasterisk" | "asterisk" | "*" => vec![EvKey::KEY_KPASTERISK],
        "kpslash" => vec![EvKey::KEY_KPSLASH],
        "kpdot" => vec![EvKey::KEY_KPDOT],
        "kpequal" => vec![EvKey::KEY_KPEQUAL],

        "f1" => vec![EvKey::KEY_F1],
        "f2" => vec![EvKey::KEY_F2],
        "f3" => vec![EvKey::KEY_F3],
        "f4" => vec![EvKey::KEY_F4],
        "f5" => vec![EvKey::KEY_F5],
        "f6" => vec![EvKey::KEY_F6],
        "f7" => vec![EvKey::KEY_F7],
        "f8" => vec![EvKey::KEY_F8],
        "f9" => vec![EvKey::KEY_F9],
        "f10" => vec![EvKey::KEY_F10],
        "f11" => vec![EvKey::KEY_F11],
        "f12" => vec![EvKey::KEY_F12],

        "space" | "spacebar" => vec![EvKey::KEY_SPACE],
        "tab" => vec![EvKey::KEY_TAB],
        "enter" | "return" => vec![EvKey::KEY_ENTER],
        "escape" | "esc" => vec![EvKey::KEY_ESC],
        "backspace" => vec![EvKey::KEY_BACKSPACE],
        "delete" | "del" => vec![EvKey::KEY_DELETE],
        "insert" | "ins" => vec![EvKey::KEY_INSERT],
        "home" => vec![EvKey::KEY_HOME],
        "end" => vec![EvKey::KEY_END],
        "pageup" => vec![EvKey::KEY_PAGEUP],
        "pagedown" => vec![EvKey::KEY_PAGEDOWN],
        "up" => vec![EvKey::KEY_UP],
        "down" => vec![EvKey::KEY_DOWN],
        "left" => vec![EvKey::KEY_LEFT],
        "right" => vec![EvKey::KEY_RIGHT],
        "capslock" => vec![EvKey::KEY_CAPSLOCK],
        "minus" | "-" => vec![EvKey::KEY_MINUS],
        "equal" | "=" => vec![EvKey::KEY_EQUAL],
        "comma" | "," => vec![EvKey::KEY_COMMA],
        "period" | "." => vec![EvKey::KEY_DOT],
        "slash" | "/" => vec![EvKey::KEY_SLASH],
        "semicolon" | ";" => vec![EvKey::KEY_SEMICOLON],
        "apostrophe" | "'" => vec![EvKey::KEY_APOSTROPHE],
        "leftbracket" | "[" => vec![EvKey::KEY_LEFTBRACE],
        "rightbracket" | "]" => vec![EvKey::KEY_RIGHTBRACE],
        "backslash" | "\\" => vec![EvKey::KEY_BACKSLASH],
        "grave" | "`" => vec![EvKey::KEY_GRAVE],

        _ => return None,
    })
}
