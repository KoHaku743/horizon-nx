//! HD rumble: steps played on the Joy-Cons' linear actuators through libnx's vibration API
//! (`switch/services/hid.h`), each band with its own amplitude and frequency. A worker thread
//! plays one list of steps at a time; a new list cuts the one playing, and the motors stop at
//! the end.
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

/// libnx `HidNpadIdType_No1` and `HidNpadIdType_Handheld`.
const PLAYER_1: u32 = 0;
const HANDHELD: u32 = 0x20;
/// libnx `HidNpadStyleTag`s with two vibration devices.
const STYLE_FULL_KEY: u32 = 1 << 0;
const STYLE_HANDHELD: u32 = 1 << 1;
const STYLE_JOY_DUAL: u32 = 1 << 2;

/// Plays `steps` now, cutting whatever plays.
pub fn play(steps: Vec<Step>) {
    static WORKER: OnceLock<Option<Mutex<Sender<Vec<Step>>>>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (send, receive) = channel();
        std::thread::Builder::new()
            .name("rumble".into())
            .stack_size(64 * 1024)
            .spawn(move || run(&receive))
            .ok()
            .map(|_| Mutex::new(send))
    });
    if let Some(send) = worker {
        let _ = send.lock().unwrap_or_else(|e| e.into_inner()).send(steps);
    }
}

fn run(receive: &Receiver<Vec<Step>>) {
    let mut motors = Motors::default();
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

/// The vibration devices of the controller the player holds, made again when it changes.
#[derive(Default)]
struct Motors {
    target: Option<(u32, u32)>,
    handles: [u32; 2],
}

impl Motors {
    fn send(&mut self, value: VibrationValue) {
        let Some(target) = active_target() else {
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

/// The npad id and style the player is holding: the handheld Joy-Con when attached, else player
/// 1's Pro Controller or Joy-Con pair. None for a single Joy-Con or no controller.
fn active_target() -> Option<(u32, u32)> {
    // SAFETY: plain libnx queries.
    let handheld = unsafe { hidGetNpadStyleSet(HANDHELD) };
    if handheld & STYLE_HANDHELD != 0 {
        return Some((HANDHELD, STYLE_HANDHELD));
    }
    let player = unsafe { hidGetNpadStyleSet(PLAYER_1) };
    if player & STYLE_FULL_KEY != 0 {
        Some((PLAYER_1, STYLE_FULL_KEY))
    } else if player & STYLE_JOY_DUAL != 0 {
        Some((PLAYER_1, STYLE_JOY_DUAL))
    } else {
        None
    }
}
