use maudio_sys::ffi as sys;

use crate::{ErrorKinds, MaudioError};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttenuationModel {
    None,
    Inverse,
    Linear,
    Exponential,
}

impl From<AttenuationModel> for sys::ma_attenuation_model {
    fn from(v: AttenuationModel) -> Self {
        match v {
            AttenuationModel::None => sys::ma_attenuation_model_ma_attenuation_model_none,
            AttenuationModel::Inverse => sys::ma_attenuation_model_ma_attenuation_model_inverse,
            AttenuationModel::Linear => sys::ma_attenuation_model_ma_attenuation_model_linear,
            AttenuationModel::Exponential => {
                sys::ma_attenuation_model_ma_attenuation_model_exponential
            }
        }
    }
}

impl TryFrom<sys::ma_attenuation_model> for AttenuationModel {
    type Error = MaudioError;

    fn try_from(v: sys::ma_attenuation_model) -> std::result::Result<Self, Self::Error> {
        match v {
            sys::ma_attenuation_model_ma_attenuation_model_none => Ok(AttenuationModel::None),
            sys::ma_attenuation_model_ma_attenuation_model_inverse => Ok(AttenuationModel::Inverse),
            sys::ma_attenuation_model_ma_attenuation_model_linear => Ok(AttenuationModel::Linear),
            sys::ma_attenuation_model_ma_attenuation_model_exponential => {
                Ok(AttenuationModel::Exponential)
            }
            _ => Err(MaudioError::new_ma_error(
                ErrorKinds::InvalidAttenuationModel,
            )),
        }
    }
}
