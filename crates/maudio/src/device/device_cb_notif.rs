use maudio_sys::ffi as sys;

use crate::{
    device::device_builder::{
        CaptureDeviceState, DuplexDeviceState, LoopBackDeviceState, PlayBackDeviceState,
    },
    pcm_frames::PcmFormat,
    util::device_notif::DeviceNotificationType,
};

pub(crate) unsafe extern "C" fn device_notification_playback_callback<F: PcmFormat, C>(
    notification: *const sys::ma_device_notification,
) {
    if notification.is_null() {
        return;
    }

    let device = (&*notification).pDevice;
    if device.is_null() {
        return;
    }

    let user_data = (*device).pUserData;
    if user_data.is_null() {
        return;
    }

    let state = user_data.cast::<PlayBackDeviceState<F, C>>();

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

pub(crate) unsafe extern "C" fn device_notification_capture_callback<F: PcmFormat, C>(
    notification: *const sys::ma_device_notification,
) {
    if notification.is_null() {
        return;
    }

    let device = (&*notification).pDevice;
    if device.is_null() {
        return;
    }

    let user_data = (*device).pUserData;
    if user_data.is_null() {
        return;
    }

    let state = user_data.cast::<CaptureDeviceState<F, C>>();

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

pub(crate) unsafe extern "C" fn device_notification_duplex_callback<F: PcmFormat, C>(
    notification: *const sys::ma_device_notification,
) {
    if notification.is_null() {
        return;
    }

    let device = (&*notification).pDevice;
    if device.is_null() {
        return;
    }

    let user_data = (*device).pUserData;
    if user_data.is_null() {
        return;
    }

    let state = user_data.cast::<DuplexDeviceState<F, C>>();

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

pub(crate) unsafe extern "C" fn device_notification_loopback_callback<F: PcmFormat, C>(
    notification: *const sys::ma_device_notification,
) {
    if notification.is_null() {
        return;
    }

    let device = (&*notification).pDevice;
    if device.is_null() {
        return;
    }

    let user_data = (*device).pUserData;
    if user_data.is_null() {
        return;
    }

    let state = user_data.cast::<LoopBackDeviceState<F, C>>();

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
