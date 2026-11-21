//! The system toast a world backup or a world that cannot be locked shows on the world list.
use super::*;

pub(super) enum WorldToast {
    Created {
        folder: String,
        bytes: u64,
    },
    Failed(String),
    /// `SystemToast.onWorldAccessFailure`: the folder of a world that couldn't be locked.
    AccessFailure(String),
    /// The Switch build's update notices (`switch_update`).
    Update(crate::switch_update::Event),
}
impl WorldToast {
    pub(super) fn text(&self) -> (Text, Text) {
        match self {
            Self::Created { folder, bytes } => (
                Text::tr("selectWorld.edit.backupCreated", &[folder]),
                Text::tr(
                    "selectWorld.edit.backupSize",
                    &[&((*bytes as f64 / 1_048_576.0).ceil() as u64)],
                ),
            ),
            Self::Failed(error) => (
                Text::translate("selectWorld.edit.backupFailed", &[]),
                Text::literal(error),
            ),
            Self::AccessFailure(folder) => (
                Text::translate("selectWorld.access_failure", &[]),
                Text::literal(folder),
            ),
            Self::Update(event) => {
                use crate::switch_update::Event;
                let (title, message) = match event {
                    Event::Available { version } => (
                        lang::get_or("horizon.update.available", "Update available"),
                        translate(
                            "horizon.update.available.message",
                            "Horizon %s: Update on the title screen",
                            &[version],
                        ),
                    ),
                    Event::Installed { version } => (
                        translate(
                            "horizon.update.installed",
                            "Horizon %s installed",
                            &[version],
                        ),
                        lang::get_or(
                            "horizon.update.installed.message",
                            "Quit Horizon to start it",
                        ),
                    ),
                    Event::Failed { reason } => (
                        lang::get_or("horizon.update.failed", "Update failed"),
                        (*reason).to_owned(),
                    ),
                    Event::Updated { version } => (
                        lang::get_or("horizon.update.updated", "Horizon updated"),
                        translate("horizon.update.updated.message", "Now on %s", &[version]),
                    ),
                };
                (Text::literal(title), Text::literal(message))
            }
        }
    }
}
