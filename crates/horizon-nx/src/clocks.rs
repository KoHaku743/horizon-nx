//! The clocks: the handheld GPU Horizon plays at, splitscreen's faster one, and the log.
//!
//! An application starts in the system's performance configurations: in handheld mode 0x00020003
//! (CPU 1020 MHz, GPU 307.2 MHz, memory 1331.2 MHz). Horizon asks apm for 0x92220008 in handheld
//! mode (GPU 460.8 MHz, memory still 1331.2 MHz), a configuration the system offers games
//! (`apmSetPerformanceConfiguration`; switchbrew "PTM services", PerformanceConfiguration; libnx
//! switch/services/apm.h, where `appletInitialize` sets apm up for an Application), and 0x00010001
//! docked (GPU 768 MHz, the system's own). `HORIZON_NX_GPU_CLOCKS` in env.txt picks another:
//! `system` keeps the system's, `max` raises handheld memory to 1600 MHz too (0x92220007).
//!
//! While splitscreen has another player in handheld mode, the GPU runs faster still, 614.4 MHz
//! by default: past every configuration apm offers, so set on the GPU's clock itself (clkrst,
//! 8.0.0+), as sys-clk does, and set again if the system puts its own back.
//! `HORIZON_NX_SPLIT_GPU_MHZ` in env.txt picks the rate (`0` leaves the GPU alone). Alone again,
//! or docked, the rate from before comes back. `[Clocks]` lines log the mode and both modes'
//! configurations at launch and whenever the system says the mode changed (`log_clocks`, from
//! `process_applet_messages`), and splitscreen's changes, so a log says which clocks its frames
//! had. The CPU boost's FastLoad mode stands in for the CPU while it is on (boost.rs).

use crate::logging::write_log_line;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// libnx `ApmPerformanceMode`: Normal is the handheld mode, Boost the docked one.
const MODE_HANDHELD: i32 = 0;
const MODE_DOCKED: i32 = 1;
/// libnx `AppletOperationMode_Console`: docked.
const OPERATION_DOCKED: i32 = 1;
/// libnx `AppletType_Application` (switch/services/applet.h).
const APPLET_TYPE_APPLICATION: i32 = 0;

/// Handheld GPU 460.8 MHz with memory at 1331.2 MHz, and with memory at 1600 MHz.
const HANDHELD_FAST: u32 = 0x9222_0008;
const HANDHELD_MAX: u32 = 0x9222_0007;
/// Docked GPU 768 MHz.
const DOCKED_FAST: u32 = 0x0001_0001;

/// switchbrew "PTM services", PerformanceConfiguration: the clocks behind each value.
const CONFIGURATIONS: [(u32, &str); 16] = [
    (0x0001_0000, "CPU 1020 / GPU 384 / memory 1600 MHz"),
    (0x0001_0001, "CPU 1020 / GPU 768 / memory 1600 MHz"),
    (0x0001_0002, "CPU 1224 / GPU 691.2 / memory 1600 MHz"),
    (0x0002_0000, "CPU 1020 / GPU 230.4 / memory 1600 MHz"),
    (0x0002_0001, "CPU 1020 / GPU 307.2 / memory 1600 MHz"),
    (0x0002_0002, "CPU 1224 / GPU 230.4 / memory 1600 MHz"),
    (0x0002_0003, "CPU 1020 / GPU 307.2 / memory 1331.2 MHz"),
    (0x0002_0004, "CPU 1020 / GPU 384 / memory 1331.2 MHz"),
    (0x0002_0005, "CPU 1020 / GPU 307.2 / memory 1065.6 MHz"),
    (0x0002_0006, "CPU 1020 / GPU 384 / memory 1065.6 MHz"),
    (0x9222_0007, "CPU 1020 / GPU 460.8 / memory 1600 MHz"),
    (0x9222_0008, "CPU 1020 / GPU 460.8 / memory 1331.2 MHz"),
    (0x9222_0009, "CPU 1785 / GPU 76.8 / memory 1600 MHz"),
    (0x9222_000A, "CPU 1785 / GPU 76.8 / memory 1331.2 MHz"),
    (0x9222_000B, "CPU 1020 / GPU 76.8 / memory 1600 MHz"),
    (0x9222_000C, "CPU 1020 / GPU 76.8 / memory 1331.2 MHz"),
];

unsafe extern "C" {
    fn appletGetAppletType() -> i32;
    fn appletGetOperationMode() -> i32;
    fn appletGetPerformanceMode() -> i32;
    fn apmSetPerformanceConfiguration(mode: i32, configuration: u32) -> u32;
    fn apmGetPerformanceConfiguration(mode: i32, configuration: *mut u32) -> u32;
    fn clkrstInitialize() -> u32;
    fn clkrstOpenSession(session: *mut ClkrstSession, module: u32, unk: u32) -> u32;
    fn clkrstSetClockRate(session: *mut ClkrstSession, hz: u32) -> u32;
    fn clkrstGetClockRate(session: *mut ClkrstSession, hz: *mut u32) -> u32;
}

/// libnx `ClkrstSession`: a `Service` (16 bytes), with room to spare.
#[repr(C, align(8))]
struct ClkrstSession([u64; 4]);
/// libnx `PcvModuleId_GPU`.
const MODULE_GPU: u32 = 0x4000_0002;
/// Splitscreen's handheld GPU rate unless env.txt says otherwise: 614.4 MHz, one of the GPU's own
/// steps.
const SPLIT_GPU_HZ: u32 = 614_400_000;
/// How often splitscreen checks the system kept its rate.
const RECHECK: Duration = Duration::from_secs(1);

fn describe(configuration: u32) -> String {
    let clocks = CONFIGURATIONS
        .iter()
        .find(|(value, _)| *value == configuration)
        .map_or("clocks unknown", |(_, clocks)| clocks);
    format!("0x{configuration:08x} ({clocks})")
}

fn is_application() -> bool {
    // SAFETY: a plain libnx call; libnx sets up the applet service before `main`.
    unsafe { appletGetAppletType() == APPLET_TYPE_APPLICATION }
}

/// One mode's configuration, as apm reports it.
fn configuration(mode: i32) -> String {
    let mut value = 0;
    // SAFETY: apm is set up for an Application (checked by the callers); `value` is written.
    let rc = unsafe { apmGetPerformanceConfiguration(mode, &mut value) };
    if rc == 0 {
        describe(value)
    } else {
        format!("unknown (apmGetPerformanceConfiguration: 0x{rc:x})")
    }
}

/// Logs the clocks and asks for the ones Horizon plays at (`HORIZON_NX_GPU_CLOCKS`, the module's
/// doc). `init` calls it once, after env.txt is applied and before the CPU boost goes on.
pub(crate) fn start_from_env() {
    if !is_application() {
        write_log_line(format_args!(
            "[Clocks] not an Application, so no apm session: the system's clocks stay\n"
        ));
        return;
    }
    log_clocks("at launch, as the system set them");
    let choice = std::env::var("HORIZON_NX_GPU_CLOCKS").ok();
    let handheld = match choice.as_deref().map(str::trim) {
        Some("system") => return,
        None | Some("fast") => HANDHELD_FAST,
        Some("max") => HANDHELD_MAX,
        Some(other) => {
            write_log_line(format_args!(
                "[Clocks] HORIZON_NX_GPU_CLOCKS={other:?} isn't fast, max or system; the system's clocks stay\n"
            ));
            return;
        }
    };
    for (mode, name, wanted) in [
        (MODE_HANDHELD, "handheld", handheld),
        (MODE_DOCKED, "docked", DOCKED_FAST),
    ] {
        // SAFETY: apm is set up for an Application (checked above).
        let rc = unsafe { apmSetPerformanceConfiguration(mode, wanted) };
        write_log_line(format_args!(
            "[Clocks] {name}: asked for {} (result 0x{rc:x})\n",
            describe(wanted)
        ));
    }
    log_clocks("after asking");
}

/// Splitscreen's GPU: the session to the clock, the rate asked for, the one from before, and when
/// it was last checked.
struct Split {
    session: ClkrstSession,
    /// No session could be opened: the rate stays the system's.
    failed: bool,
    on: Option<(u32, u32)>,
    checked: Instant,
}
static SPLIT: Mutex<Option<Split>> = Mutex::new(None);

/// `HORIZON_NX_SPLIT_GPU_MHZ`: splitscreen's handheld GPU rate in Hz, 0 to leave it alone.
fn split_rate() -> u32 {
    match std::env::var("HORIZON_NX_SPLIT_GPU_MHZ") {
        Err(_) => SPLIT_GPU_HZ,
        Ok(mhz) => mhz
            .trim()
            .parse::<f64>()
            .map_or(0, |mhz| (mhz * 1_000_000.0).round().clamp(0.0, 1e9) as u32),
    }
}

/// Splitscreen: while another player is in (`on`) in handheld mode, the GPU runs at
/// `split_rate`, set again within a second if the system puts its own back; otherwise the rate
/// from before comes back. Called every frame.
pub fn splitscreen_clocks(on: bool) {
    let mut split = SPLIT.lock().unwrap_or_else(|e| e.into_inner());
    if split.is_none() && !on {
        return;
    }
    // SAFETY: a plain libnx getter of the applet state it keeps.
    let docked = unsafe { appletGetOperationMode() } == OPERATION_DOCKED;
    let rate = if on && !docked { split_rate() } else { 0 };
    if split.is_none() {
        if rate == 0 || !is_application() {
            return;
        }
        let mut session = ClkrstSession([0; 4]);
        // SAFETY: clkrst comes up once and stays for the process; `session` has libnx's room.
        let rc = unsafe {
            match clkrstInitialize() {
                0 => clkrstOpenSession(&mut session, MODULE_GPU, 3),
                rc => rc,
            }
        };
        if rc != 0 {
            write_log_line(format_args!(
                "[Clocks] splitscreen: no session to the GPU's clock (0x{rc:x}); its rate stays\n"
            ));
            *split = Some(Split {
                session,
                failed: true,
                on: None,
                checked: Instant::now(),
            });
            return;
        }
        *split = Some(Split {
            session,
            failed: false,
            on: None,
            checked: Instant::now(),
        });
    }
    let Some(split) = split.as_mut().filter(|split| !split.failed) else {
        return;
    };
    let mut now = 0;
    match (split.on, rate) {
        (None, 0) => {}
        (None, wanted) => {
            // SAFETY: the session was opened above.
            unsafe { clkrstGetClockRate(&mut split.session, &mut now) };
            // SAFETY: as above.
            let rc = unsafe { clkrstSetClockRate(&mut split.session, wanted) };
            write_log_line(format_args!(
                "[Clocks] splitscreen: GPU {} -> {} MHz (result 0x{rc:x})\n",
                f64::from(now) / 1e6,
                f64::from(wanted) / 1e6
            ));
            split.on = Some((wanted, now));
            split.checked = Instant::now();
        }
        // Docked, the system has set its own (faster) clocks: they stay.
        (Some(_), 0) if docked => {
            write_log_line(format_args!(
                "[Clocks] splitscreen over: docked, the system's GPU rate stays\n"
            ));
            split.on = None;
        }
        (Some((_, before)), 0) => {
            // A rate that couldn't be read stays the splitscreen one until the system sets its own.
            let rc = match before {
                0 => 0,
                // SAFETY: as above.
                before => unsafe { clkrstSetClockRate(&mut split.session, before) },
            };
            write_log_line(format_args!(
                "[Clocks] splitscreen over: GPU back to {} MHz (result 0x{rc:x})\n",
                f64::from(before) / 1e6
            ));
            split.on = None;
        }
        (Some((wanted, _)), _) if split.checked.elapsed() >= RECHECK => {
            split.checked = Instant::now();
            // SAFETY: as above.
            unsafe { clkrstGetClockRate(&mut split.session, &mut now) };
            if now != wanted {
                // SAFETY: as above.
                unsafe { clkrstSetClockRate(&mut split.session, wanted) };
            }
        }
        _ => {}
    }
}

/// Logs whether the console is docked and both modes' configurations. From `start_from_env`, and
/// from `process_applet_messages` when the system says the operation or performance mode changed.
pub(crate) fn log_clocks(when: &str) {
    if !is_application() {
        return;
    }
    // SAFETY: plain libnx getters of the applet state it keeps.
    let (operation, performance) =
        unsafe { (appletGetOperationMode(), appletGetPerformanceMode()) };
    let mode = if operation == OPERATION_DOCKED {
        "docked"
    } else {
        "handheld"
    };
    write_log_line(format_args!(
        "[Clocks] {when}: {mode} (performance mode {performance}); handheld configuration {}, docked {}\n",
        configuration(MODE_HANDHELD),
        configuration(MODE_DOCKED),
    ));
}
