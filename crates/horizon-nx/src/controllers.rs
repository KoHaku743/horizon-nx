//! The system's controller screen (libnx `hidLaShowControllerSupport`, the "Controllers" library
//! applet games show before local multiplayer): players connect, pair and order their
//! controllers there, each as an npad player.

/// libnx `HidLaControllerSupportArg` (8.0.0+; libnx converts it for older firmware).
#[repr(C)]
struct Arg {
    player_count_min: i8,
    player_count_max: i8,
    enable_take_over_connection: u8,
    enable_left_justify: u8,
    enable_permit_joy_dual: u8,
    enable_single_mode: u8,
    enable_identification_color: u8,
    identification_color: [[u8; 4]; 8],
    enable_explain_text: u8,
    explain_text: [[u8; 0x81]; 8],
}

/// libnx `HidLaControllerSupportResultInfo`.
#[repr(C)]
#[derive(Default)]
struct ResultInfo {
    player_count: i8,
    pad: [u8; 3],
    selected_id: u32,
}

unsafe extern "C" {
    fn hidLaCreateControllerSupportArg(arg: *mut Arg);
    fn hidLaShowControllerSupport(result: *mut ResultInfo, arg: *const Arg) -> u32;
    fn hidLaShowControllerSupportForSystem(
        result: *mut ResultInfo,
        arg: *const Arg,
        flag: bool,
    ) -> u32;
    fn hidGetNpadJoyHoldType(hold: *mut u32) -> u32;
    fn hidSetNpadJoyHoldType(hold: u32) -> u32;
    fn hidGetSupportedNpadStyleSet(style_set: *mut u32) -> u32;
}

/// libnx `HidNpadJoyHoldType_Horizontal` (the default is vertical).
const HOLD_HORIZONTAL: u32 = 1;

/// Shows the controller screen for 1 to `max_players` players, a Pro Controller or a pair of
/// Joy-Con each, and returns once it closes (the game waits meanwhile): how many players
/// connected, or None when it couldn't open or was cancelled.
///
/// The screen closed at once with an error in every first try on hardware (libnx
/// LibAppletBadExit: the applet answered, with a failure), for a reason not yet known. Until it
/// is, the attempts that could help follow in turn, each in the log with the hold type and
/// styles HID hands the applet: as games call it, then with the Joy-Con held sideways (HID's
/// default is upright), then as the system's own call (`ForSystem`).
pub fn show(max_players: u8) -> Option<u8> {
    let mut arg = Arg {
        player_count_min: 0,
        player_count_max: 0,
        enable_take_over_connection: 0,
        enable_left_justify: 0,
        enable_permit_joy_dual: 0,
        enable_single_mode: 0,
        enable_identification_color: 0,
        identification_color: [[0; 4]; 8],
        enable_explain_text: 0,
        explain_text: [[0; 0x81]; 8],
    };
    // SAFETY: libnx fills the defaults into a struct of its layout.
    unsafe { hidLaCreateControllerSupportArg(&mut arg) };
    arg.player_count_min = 1;
    arg.player_count_max = i8::try_from(max_players.clamp(1, 4)).unwrap_or(4);
    let (mut hold, mut styles) = (u32::MAX, 0u32);
    // SAFETY: plain libnx queries writing one u32 each.
    let (hold_rc, styles_rc) = unsafe {
        (
            hidGetNpadJoyHoldType(&mut hold),
            hidGetSupportedNpadStyleSet(&mut styles),
        )
    };
    log::info!(
        "Horizon: the controller screen for 1 to {} players: hold type {hold} (0x{hold_rc:x}), \
         supported styles 0x{styles:x} (0x{styles_rc:x})",
        arg.player_count_max
    );
    let attempts: [(&str, fn(&mut ResultInfo, &Arg) -> u32); 3] = [
        ("as games call it", |result, arg| {
            // SAFETY: both structs have libnx's layout; the call blocks until the applet closes.
            unsafe { hidLaShowControllerSupport(result, arg) }
        }),
        ("with the Joy-Con held sideways", |result, arg| {
            // SAFETY: as above; the hold type is a plain HID setting.
            unsafe {
                let set = hidSetNpadJoyHoldType(HOLD_HORIZONTAL);
                if set != 0 {
                    return set;
                }
                hidLaShowControllerSupport(result, arg)
            }
        }),
        ("as the system's call", |result, arg| {
            // SAFETY: as above.
            unsafe { hidLaShowControllerSupportForSystem(result, arg, false) }
        }),
    ];
    for (how, attempt) in attempts {
        let mut result = ResultInfo::default();
        let rc = attempt(&mut result, &arg);
        if rc == 0 {
            log::info!(
                "Horizon: the controller screen ({how}) closed with {} player(s)",
                result.player_count
            );
            return u8::try_from(result.player_count).ok();
        }
        log::warn!(
            "Horizon: the controller screen ({how}) did not open or was closed (0x{rc:x}, \
             {} player(s))",
            result.player_count
        );
    }
    None
}
