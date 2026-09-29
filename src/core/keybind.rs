use evdev::{Device, EventType, InputEventKind, Key as EvKey};
use futures::FutureExt;
use futures::{channel::mpsc, select, StreamExt};
use gpui_kit::Context;
use std::collections::HashSet;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::app::SoundboardApp;

#[derive(Clone)]
pub struct GlobalShortcutManager {
    tx: mpsc::UnboundedSender<Command>,
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
            let worker = thread::spawn(move || {
                if let Err(err) = run_evdev_loop(shortcuts, thread_stop, event_tx) {
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

        Self { tx }
    }

    pub fn reload(&self) {
        let _ = self.tx.unbounded_send(Command::Reload);
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

        "0" => vec![EvKey::KEY_0, EvKey::KEY_KP0],
        "1" => vec![EvKey::KEY_1, EvKey::KEY_KP1],
        "2" => vec![EvKey::KEY_2, EvKey::KEY_KP2],
        "3" => vec![EvKey::KEY_3, EvKey::KEY_KP3],
        "4" => vec![EvKey::KEY_4, EvKey::KEY_KP4],
        "5" => vec![EvKey::KEY_5, EvKey::KEY_KP5],
        "6" => vec![EvKey::KEY_6, EvKey::KEY_KP6],
        "7" => vec![EvKey::KEY_7, EvKey::KEY_KP7],
        "8" => vec![EvKey::KEY_8, EvKey::KEY_KP8],
        "9" => vec![EvKey::KEY_9, EvKey::KEY_KP9],

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
        "enter" | "return" => vec![EvKey::KEY_ENTER, EvKey::KEY_KPENTER],
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
        "minus" | "-" => vec![EvKey::KEY_MINUS, EvKey::KEY_KPMINUS],
        "equal" | "=" => vec![EvKey::KEY_EQUAL, EvKey::KEY_KPEQUAL],
        "comma" | "," => vec![EvKey::KEY_COMMA],
        "period" | "." => vec![EvKey::KEY_DOT, EvKey::KEY_KPDOT],
        "slash" | "/" => vec![EvKey::KEY_SLASH, EvKey::KEY_KPSLASH],
        "asterisk" | "*" => vec![EvKey::KEY_KPASTERISK],
        "plus" | "+" => vec![EvKey::KEY_KPPLUS],
        "semicolon" | ";" => vec![EvKey::KEY_SEMICOLON],
        "apostrophe" | "'" => vec![EvKey::KEY_APOSTROPHE],
        "leftbracket" | "[" => vec![EvKey::KEY_LEFTBRACE],
        "rightbracket" | "]" => vec![EvKey::KEY_RIGHTBRACE],
        "backslash" | "\\" => vec![EvKey::KEY_BACKSLASH],
        "grave" | "`" => vec![EvKey::KEY_GRAVE],

        _ => return None,
    })
}
