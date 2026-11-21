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
}

/// Shows the controller screen for 1 to `max_players` players, a Pro Controller or a pair of
/// Joy-Con each, and returns once it closes (the game waits meanwhile): how many players
/// connected, or None when it couldn't open or was cancelled.
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
    let mut result = ResultInfo::default();
    // SAFETY: both structs have libnx's layout; the call blocks until the applet closes.
    let rc = unsafe { hidLaShowControllerSupport(&mut result, &arg) };
    if rc != 0 {
        log::warn!("Horizon: the controller screen did not open or was closed (0x{rc:x})");
        return None;
    }
    log::info!(
        "Horizon: the controller screen closed with {} player(s)",
        result.player_count
    );
    u8::try_from(result.player_count).ok()
}
