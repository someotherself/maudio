use maudio_sys::ffi as sys;

use crate::{engine::process_cb::ProcessState, util::device_notif::DeviceNotificationType};

pub(crate) unsafe extern "C" fn engine_notification_callback(
    notification: *const sys::ma_device_notification,
) {
    if notification.is_null() {
        return;
    }

    let device = (&*notification).pDevice;
    if device.is_null() {
        return;
    }

    let engine = (*device).pUserData.cast::<sys::ma_engine>();
    if engine.is_null() {
        return;
    }

    let state = (*engine).pProcessUserData.cast::<ProcessState>();
    if state.is_null() {
        return;
    }

    let mask = match (&*notification).type_ {
        sys::ma_device_notification_type_ma_device_notification_type_started => {
            DeviceNotificationType::Started.bit()
        }
        sys::ma_device_notification_type_ma_device_notification_type_stopped => {
            DeviceNotificationType::Stopped.bit()
        }
        sys::ma_device_notification_type_ma_device_notification_type_rerouted => {
            DeviceNotificationType::Rerouted.bit()
        }
        sys::ma_device_notification_type_ma_device_notification_type_interruption_began => {
            DeviceNotificationType::InterruptionBegan.bit()
        }
        sys::ma_device_notification_type_ma_device_notification_type_interruption_ended => {
            DeviceNotificationType::InterruptionEnded.bit()
        }
        sys::ma_device_notification_type_ma_device_notification_type_unlocked => {
            DeviceNotificationType::Unlocked.bit()
        }
        _ => 0,
    };
    (*state).state_notif.store_notifications(mask);
}
