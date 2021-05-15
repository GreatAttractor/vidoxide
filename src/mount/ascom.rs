//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! ASCOM telescope (mount) driver.
//!
//! NOTE: this code has been only tested with a 2014 HEQ5 mount via EQASCOM.
//!

use winapi;
use uuid::Uuid;
use std::os::windows::ffi::OsStrExt;
use crate::mount::{Axis, Mount, MountError, SIDEREAL_RATE};

#[repr(i32)]
#[derive(Debug)]
enum AlignmentModes {
    algAltAz = 0,
    algPolar = 1,
    algGermanPolar = 2
}

#[repr(i32)]
enum TelescopeAxes {
    axisPrimary = 0,
    axisSecondary = 1,
    axisTertiary = 2
}

#[repr(u16)]
#[derive(Debug, PartialEq)]
enum VariantBool {
    FALSE = 0x0000,
    TRUE = 0xFFFF
}

macro_rules! checked_call {
    ($func_call:expr) => {
        match unsafe { $func_call } {
            winapi::shared::winerror::S_OK => Ok(()),
            error => Err(error)
        }
    }
}

fn iid_from_string(s: &str) -> winapi::shared::guiddef::IID {
    let uuid = Uuid::parse_str(s).unwrap();
    let fields = uuid.as_fields();
    winapi::shared::guiddef::IID{
        Data1: fields.0,
        Data2: fields.1,
        Data3: fields.2,
        Data4: *fields.3
    }
}

#[derive(Debug)]
pub struct AscomError(String);

impl std::convert::From<winapi::um::winnt::HRESULT> for AscomError {
    fn from(e: winapi::um::winnt::HRESULT) -> AscomError {
        AscomError(format!("Error code: 0x{:X}.", e))
    }
}

impl std::convert::From<AscomError> for MountError {
    fn from(e: AscomError) -> MountError {
        MountError::AscomError(e)
    }
}

// Created manually based on "ASCOMPlatform/Interfaces/Master Interfaces/AscomMasterInterfaces.idl"
// (https://github.com/ASCOMInitiative/ASCOMPlatform). UUID: EF0C67AD-A9D3-4f7b-A635-CD2095517633.
#[repr(C)]
struct ITelescopeVTbl {
    pub parent: winapi::um::oaidl::IDispatchVtbl,

    AlignmentMode: unsafe extern "system" fn(This: *mut ITelescope, retval: *mut AlignmentModes) -> winapi::um::winnt::HRESULT,

    dummy01: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy02: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy03: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy04: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy05: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy06: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy07: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy08: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy09: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy10: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy11: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy12: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy13: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy14: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy15: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy16: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy17: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy18: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy19: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy20: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy21: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy22: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,

    Connected: unsafe extern "system" fn(This: *mut ITelescope, is_connected: *mut VariantBool) -> winapi::um::winnt::HRESULT,

    dummy24: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy25: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy26: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy27: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy28: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy29: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy30: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy31: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy32: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy33: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy34: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy35: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy36: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy37: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy38: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy39: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy40: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy41: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy42: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy43: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy44: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy45: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy46: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy47: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy48: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy49: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy50: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy51: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy52: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy53: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy54: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy55: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy56: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy57: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy58: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy59: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy60: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy61: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy62: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy63: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy64: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy65: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy66: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy67: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy68: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy69: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,

    CanMoveAxis: unsafe extern "system" fn(This: *mut ITelescope, Axis: TelescopeAxes, can_move: *mut VariantBool) -> winapi::um::winnt::HRESULT,

    dummy71: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy72: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,

    MoveAxis: unsafe extern "system" fn(This: *mut ITelescope, Axis: TelescopeAxes, Rate: f64) -> winapi::um::winnt::HRESULT,

    dummy74: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy75: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy76: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy77: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy78: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy79: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy81: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy82: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy83: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy84: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy85: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy86: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
    dummy87: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,

    Unpark: unsafe extern "system" fn(This: *mut ITelescope) -> winapi::um::winnt::HRESULT,
}

#[repr(C)]
struct ITelescope {
    lpVtbl: *const ITelescopeVTbl
}

impl std::ops::Deref for ITelescope {
    type Target = winapi::um::oaidl::IDispatch;

    #[inline]
    fn deref(&self) -> &winapi::um::oaidl::IDispatch {
        unsafe { &*(self as *const ITelescope as *const winapi::um::oaidl::IDispatch) }
    }
}

pub struct Ascom {
    telescope: *mut ITelescope,
    driver: String
}

impl Drop for Ascom {
    fn drop(&mut self) {
        checked_call!(unsafe { ((*(*self.telescope).lpVtbl).MoveAxis)(
            self.telescope,
            TelescopeAxes::axisPrimary,
            0.0
        ) });

        checked_call!(unsafe { ((*(*self.telescope).lpVtbl).MoveAxis)(
            self.telescope,
            TelescopeAxes::axisSecondary,
            0.0
        ) });

        unsafe { (*(self.telescope as *mut winapi::um::oaidl::IDispatch)).Release(); }
    }
}

impl Ascom {
    /// Creates new ASCOM telescope instance.
    ///
    /// # Parameters
    ///
    /// * `progid` - ProgID of telescope (e.g., "EQMOD.Telescope").
    ///
    pub fn new(progid: &str) -> Result<Ascom, AscomError> {
        // Note: we do not call winapi::um::objbase::CoInitialize and winapi::um::combaseapi::CoUninitialize,
        // since GTK already does that.

        let mut telescope_clsid = winapi::shared::guiddef::IID::default();
        let telescope_string = std::ffi::OsStr::new(progid).encode_wide().collect::<Vec<u16>>();
        checked_call!(unsafe { winapi::um::combaseapi::CLSIDFromProgID(
            telescope_string.as_ptr(),
            &mut telescope_clsid
        ) })?;

        let itelescope_iid = iid_from_string("EF0C67AD-A9D3-4F7B-A635-CD2095517633");

        let mut telescope: *mut ITelescope = std::ptr::null_mut();

        checked_call!(winapi::um::combaseapi::CoCreateInstance(
            &telescope_clsid,
            std::ptr::null_mut(),
            winapi::shared::wtypesbase::CLSCTX_INPROC_SERVER | winapi::shared::wtypesbase::CLSCTX_LOCAL_SERVER,
            &itelescope_iid,
            &mut telescope as *mut _ as *mut *mut winapi::ctypes::c_void
        ))?;

        let mut is_connected = VariantBool::FALSE;
        checked_call!(unsafe { ((*(*telescope).lpVtbl).Connected)(telescope, &mut is_connected as *mut _) });
        if is_connected == VariantBool::FALSE {
            return Err(AscomError(
                "Connection to mount is not active. Try using the ASCOM configuration dialog of the driver.".to_string()
            ));
        }

        checked_call!(unsafe { ((*(*telescope).lpVtbl).Unpark)(telescope) });

        Ok(Ascom{ telescope, driver: progid.to_string() })
    }
}

fn ascom_axis_from(axis: Axis) -> TelescopeAxes {
    match axis {
        Axis::RA => TelescopeAxes::axisPrimary,
        Axis::Dec => TelescopeAxes::axisSecondary
    }
}

impl Mount for Ascom {
    fn get_info(&self) -> Result<String, MountError> {
        Ok(format!("ASCOM – {}", self.driver))
    }

    fn set_motion(&mut self, axis: Axis, speed: f64) -> Result<(), MountError> {
        checked_call!(unsafe { ((*(*self.telescope).lpVtbl).MoveAxis)(
            self.telescope,
            ascom_axis_from(axis),
            speed * 180.0 / std::f64::consts::PI
        ) });

        Ok(())
    }

    fn stop_motion(&mut self, axis: Axis) -> Result<(), MountError> {
        checked_call!(unsafe { ((*(*self.telescope).lpVtbl).MoveAxis)(
            self.telescope,
            ascom_axis_from(axis),
            0.0
        ) });

        Ok(())
    }

    fn get_motion_speed(&self, axis: Axis) -> Result<f64, MountError> {
        unimplemented!();
    }
}
