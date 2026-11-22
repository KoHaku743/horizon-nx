//! HD rumble: steps played on the Joy-Cons' linear actuators through libnx's vibration API
//! (`switch/services/hid.h`), each band with its own amplitude and frequency. Each controller
//! (the handheld Joy-Con, then players 1 to 4: splitscreen's) has a worker thread
//! that plays one list of steps at a time; a new list cuts the one playing, and the motors stop
//! at the end.
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// One step: the low and high band's amplitude (0..1) and frequency (Hz), and how long it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub amp_low: f32,
    pub freq_low: f32,
    pub amp_high: f32,
    pub freq_high: f32,
    pub millis: u32,
}

/// libnx `HidVibrationValue`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct VibrationValue {
    amp_low: f32,
    freq_low: f32,
    amp_high: f32,
    freq_high: f32,
}

/// Silence, at the bands' resting frequencies (libnx's own vibration example).
const STILL: VibrationValue = VibrationValue {
    amp_low: 0.0,
    freq_low: 160.0,
    amp_high: 0.0,
    freq_high: 320.0,
};

unsafe extern "C" {
    fn hidGetNpadStyleSet(id: u32) -> u32;
    fn hidInitializeVibrationDevices(
        handles: *mut u32,
        total_handles: i32,
        id: u32,
        style: u32,
    ) -> u32;
    fn hidSendVibrationValues(
        handles: *const u32,
        values: *const VibrationValue,
        count: i32,
    ) -> u32;
}

/// libnx `HidNpadIdType_Handheld`.
const HANDHELD: u32 = 0x20;
/// libnx `HidNpadStyleTag`s with two vibration devices.
const STYLE_FULL_KEY: u32 = 1 << 0;
const STYLE_HANDHELD: u32 = 1 << 1;
const STYLE_JOY_DUAL: u32 = 1 << 2;

/// The controllers that can vibrate, as the controller reader numbers them: the handheld Joy-Con
/// (0), then players 1 to 4.
const PADS: usize = 5;

/// Plays `steps` now on gamepad `pad`'s controller (0 = the handheld Joy-Con, 1 to 4 = players 1
/// to 4, as the controller reader numbers them), cutting whatever plays there.
pub fn play(pad: usize, steps: Vec<Step>) {
    type Worker = OnceLock<Option<Mutex<Sender<Vec<Step>>>>>;
    static WORKERS: [Worker; PADS] = [const { OnceLock::new() }; PADS];
    let Some(worker) = WORKERS.get(pad) else {
        return;
    };
    let worker = worker.get_or_init(|| {
        let (send, receive) = channel();
        std::thread::Builder::new()
            .name(format!("rumble {}", pad + 1))
            .stack_size(64 * 1024)
            .spawn(move || run(pad, &receive))
            .ok()
            .map(|_| Mutex::new(send))
    });
    if let Some(send) = worker {
        let _ = send.lock().unwrap_or_else(|e| e.into_inner()).send(steps);
    }
}

fn run(pad: usize, receive: &Receiver<Vec<Step>>) {
    let mut motors = Motors {
        pad,
        target: None,
        handles: [0; 2],
    };
    while let Ok(mut steps) = receive.recv() {
        'list: loop {
            for step in &steps {
                motors.send(VibrationValue {
                    amp_low: step.amp_low.clamp(0.0, 1.0),
                    freq_low: step.freq_low,
                    amp_high: step.amp_high.clamp(0.0, 1.0),
                    freq_high: step.freq_high,
                });
                match receive.recv_timeout(Duration::from_millis(u64::from(step.millis))) {
                    Ok(next) => {
                        steps = next;
                        continue 'list;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        motors.send(STILL);
                        return;
                    }
                }
            }
            motors.send(STILL);
            break;
        }
    }
}

/// The vibration devices of gamepad `pad`'s controller, made again when it changes.
struct Motors {
    pad: usize,
    target: Option<(u32, u32)>,
    handles: [u32; 2],
}

impl Motors {
    fn send(&mut self, value: VibrationValue) {
        let Some(target) = target(self.pad) else {
            self.target = None;
            return;
        };
        if self.target != Some(target) {
            let (id, style) = target;
            // SAFETY: libnx writes two handles into the array it is given.
            if unsafe { hidInitializeVibrationDevices(self.handles.as_mut_ptr(), 2, id, style) }
                != 0
            {
                return;
            }
            self.target = Some(target);
        }
        let values = [value; 2];
        // SAFETY: two initialized handles and two values.
        unsafe { hidSendVibrationValues(self.handles.as_ptr(), values.as_ptr(), 2) };
    }
}

/// The npad id and style gamepad `pad` vibrates: the handheld Joy-Con for gamepad 0, players 1
/// to 4 for the others. For a player, a Pro Controller or Joy-Con pair: None for a single
/// Joy-Con or no controller.
fn target(pad: usize) -> Option<(u32, u32)> {
    if pad == 0 {
        // SAFETY: a plain libnx query.
        let handheld = unsafe { hidGetNpadStyleSet(HANDHELD) };
        return (handheld & STYLE_HANDHELD != 0).then_some((HANDHELD, STYLE_HANDHELD));
    }
    // libnx `HidNpadIdType_No1` to `No8` are 0 to 7.
    let id = u32::try_from(pad - 1).ok()?;
    // SAFETY: a plain libnx query.
    let style = unsafe { hidGetNpadStyleSet(id) };
    if style & STYLE_FULL_KEY != 0 {
        Some((id, STYLE_FULL_KEY))
    } else if style & STYLE_JOY_DUAL != 0 {
        Some((id, STYLE_JOY_DUAL))
    } else {
        None
    }
}
