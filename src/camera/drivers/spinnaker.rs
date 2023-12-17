//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2021-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Spinnaker camera driver.
//!

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case
)]

use crate::camera::*;
use ga_image;
use ga_image::Image;
use libspinnaker_sys::*;
use std::sync::Arc;

#[derive(Debug)]
pub enum SpinnakerError {
    Internal(_spinError),
    Error(String)
}

impl From<SpinnakerError> for CameraError {
    fn from(spinnaker_error: SpinnakerError) -> CameraError {
        CameraError::SpinnakerError(spinnaker_error)
    }
}

macro_rules! checked_call {
    ($func_call:expr) => {
        #[allow(non_upper_case_globals)]
        match unsafe { $func_call } {
            _spinError_SPINNAKER_ERR_SUCCESS => (),
            error => return Err(SpinnakerError::Internal(error).into())
        }
    }
}

/// Index in list of cameras.
type SpinnakerCameraId = usize;

impl From<CameraId> for SpinnakerCameraId {
    fn from(id: CameraId) -> SpinnakerCameraId {
        id.id1 as usize
    }
}

impl From<SpinnakerCameraId> for CameraId  {
    fn from(id: SpinnakerCameraId) -> CameraId {
        CameraId{ id1: id as u64, id2: 0 }
    }
}

/// Common GenICam camera feature names (Standard Features Naming Convention 1.5.1).
mod genicam {
    pub const ACQUISITION_FRAME_RATE:                         &'static str = "AcquisitionFrameRate";
    pub const DEVICE_MODEL_NAME:                              &'static str = "DeviceModelName";
    pub const DEVICE_TEMPERATURE:                             &'static str = "DeviceTemperature";
    pub const DEVICE_VENDOR_NAME:                             &'static str = "DeviceVendorName";
    pub const EXPOSURE_AUTO:                                  &'static str = "ExposureAuto";
    pub const EXPOSURE_TIME:                                  &'static str = "ExposureTime";
    pub const GAIN_AUTO:                                      &'static str = "GainAuto";
    pub const GAIN:                                           &'static str = "Gain";
    pub const GAMMA:                                          &'static str = "Gamma";
    pub const HEIGHT_MAX:                                     &'static str = "HeightMax";
    pub const HEIGHT:                                         &'static str = "Height";
    pub const OFFSET_X:                                       &'static str = "OffsetX";
    pub const OFFSET_Y:                                       &'static str = "OffsetY";
    pub const PIXEL_FORMAT:                                   &'static str = "PixelFormat";
    pub const ROOT:                                           &'static str = "Root";
    pub const WIDTH_MAX:                                      &'static str = "WidthMax";
    pub const WIDTH:                                          &'static str = "Width";
}

/// FLIR-specific camera feature names (glimpsed in CM3-U3-13S2M, BFS-U3-16S2M cameras).
mod flir {
    pub const ACQUISITION_FRAME_RATE_AUTO:                    &'static str = "AcquisitionFrameRateAuto";
    pub const ACQUISITION_FRAME_RATE_ENABLE:                  &'static str = "AcquisitionFrameRateEnable";
    pub const ACQUISITION_FRAME_RATE_ENABLED:                 &'static str = "AcquisitionFrameRateEnabled";
    pub const GAMMA_ENABLE:                                   &'static str = "GammaEnable";
    pub const GAMMA_ENABLED:                                  &'static str = "GammaEnabled";
    pub const ADC_BIT_DEPTH:                                  &'static str = "AdcBitDepth";
}

/// Wrappers of Spinnaker objects.
mod spin {
    use std::cell::RefCell;
    use libspinnaker_sys::*;
    use super::{CameraError, ControlAccessMode, SpinnakerError};

    /// Entry of enumeration node.
    ///
    /// We keep track of both, because on (at least) BFS-U3-16S2M & CM3-U3-13S2M, some nodes cannot be used with
    /// `spinEnumerationGet/SetEnumValue`; and for the pixel format node, it is `spinEnumerationGetEnumValue`'s result
    /// which can be cast to `_spinPixelFormatEnums`.
    ///
    pub struct EnumEntry {
        /// Value used with `spinEnumerationEntryGetIntValue`.
        opaque: i64,
        /// Value used with `spinEnumerationEntryGetEnumValue`.
        actual: Option<size_t>,
        symbolic: String
    }

    impl EnumEntry {
        pub fn opaque(&self) -> i64 { self.opaque }
        pub fn actual(&self) -> Option<size_t> { self.actual }
        pub fn symbolic(&self) -> &str { &self.symbolic }
    }

    /// Calls the string `getter` on `object`.
    pub fn read_string<Object>(
        object: Object,
        getter: unsafe extern "C" fn(Object, *mut ::std::os::raw::c_char, *mut size_t) -> spinError
    ) -> Result<String, CameraError> {
        const MAX_BUF_LEN: usize = 256;
        let mut buf: [std::os::raw::c_char; MAX_BUF_LEN] = [0; MAX_BUF_LEN];
        let mut returned_buf_len: size_t = MAX_BUF_LEN as size_t;
        checked_call!(getter(object, (&mut buf).as_mut_ptr(), &mut returned_buf_len));
        assert!(returned_buf_len as usize <= MAX_BUF_LEN);
        if returned_buf_len == 0 {
            return Ok("".to_string());
        }

        let bytes: Vec<u8> = buf[..returned_buf_len as usize - 1].iter().map(|signed| *signed as u8).collect();
        Ok(String::from_utf8(bytes).unwrap())
    }

    /// Calls the value `getter` on `object`.
    fn read_value<Value>(
        object: *mut std::os::raw::c_void,
        getter: unsafe extern "C" fn(*mut std::os::raw::c_void, *mut Value) -> spinError
    ) -> Result<Value, CameraError> {
        let mut value = std::mem::MaybeUninit::uninit();
        checked_call!(getter(object, value.as_mut_ptr()));
        let value = unsafe { value.assume_init() };
        Ok(value)
    }

    pub struct System {
        handle: *mut std::os::raw::c_void
    }

    impl System {
        pub fn new() -> Result<System, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinSystemGetInstance(&mut handle));
            Ok(System{ handle })
        }

        pub fn get(&mut self) -> *mut std::os::raw::c_void {
            self.handle
        }
    }

    impl Drop for System {
        fn drop(&mut self) {
            unsafe { spinSystemReleaseInstance(self.handle); }
        }
    }

    pub struct CameraList {
        handle: *mut std::os::raw::c_void
    }

    impl CameraList {
        pub fn new() -> Result<CameraList, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinCameraListCreateEmpty(&mut handle));
            Ok(CameraList{ handle })
        }

        pub fn get(&mut self) -> *mut std::os::raw::c_void {
            self.handle
        }

        pub fn num_cameras(&self) -> Result<usize, CameraError> {
            let result = read_value::<size_t>(self.handle, spinCameraListGetSize)?;
            Ok(result as usize)
        }

        pub fn camera(&self, index: usize) -> Result<Camera, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinCameraListGet(self.handle, index as size_t, &mut handle));
            Ok(Camera{ handle, initialized: false })
        }
    }

    impl Drop for CameraList {
        fn drop(&mut self) {
            unsafe { spinCameraListDestroy(self.handle); }
        }
    }

    enum ImageDropMode {
        Release,
        Destroy
    }

    pub struct Image {
        handle: *mut std::os::raw::c_void,
        drop_mode: ImageDropMode
    }

    impl Image {
        pub fn status(&self) -> Result<spinImageStatus, CameraError> {
            read_value(self.handle, spinImageGetStatus)
        }

        pub fn pixel_format(&self) -> Result<spinPixelFormatEnums, CameraError> {
            read_value(self.handle, spinImageGetPixelFormat)
        }

        pub fn width(&self) -> Result<size_t, CameraError> {
            read_value(self.handle, spinImageGetWidth)
        }

        pub fn height(&self) -> Result<size_t, CameraError> {
            read_value(self.handle, spinImageGetHeight)
        }

        pub fn stride(&self) -> Result<size_t, CameraError> {
            read_value(self.handle, spinImageGetStride)
        }

        pub fn data_ptr(&self) -> Result<*const ::std::os::raw::c_void, CameraError> {
            let value = read_value(self.handle, spinImageGetData)?;
            Ok(value)
        }

        pub fn data_size(&self) -> Result<size_t, CameraError> {
            read_value(self.handle, spinImageGetBufferSize)
        }
    }

    impl Drop for Image {
        fn drop(&mut self) {
            match self.drop_mode {
                ImageDropMode::Release => unsafe { spinImageRelease(self.handle); },
                ImageDropMode::Destroy => unsafe { spinImageDestroy(self.handle); }
            }
        }
    }

    pub struct Camera {
        handle: *mut std::os::raw::c_void,
        initialized: bool
    }

    impl Camera {
        pub fn next_image(&self) -> Result<Image, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinCameraGetNextImageEx(self.handle, 5000, &mut handle));
            Ok(Image{ handle, drop_mode: ImageDropMode::Release })
        }

        pub fn init(&mut self) -> Result<(), CameraError> {
            checked_call!(spinCameraInit(self.handle));
            self.initialized = true;
            Ok(())
        }

        pub fn deinit(&mut self) -> Result<(), CameraError> {
            checked_call!(spinCameraDeInit(self.handle));
            self.initialized = false;
            Ok(())
        }

        pub fn device_node_map(&self) -> Result<NodeMap, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinCameraGetTLDeviceNodeMap(self.handle, &mut handle));
            Ok(NodeMap{ handle })
        }

        pub fn stream_node_map(&self) -> Result<NodeMap, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinCameraGetTLStreamNodeMap(self.handle, &mut handle));
            Ok(NodeMap{ handle })
        }

        /// Requires initialization via `init`.
        pub fn genicam_node_map(&self) -> Result<NodeMap, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinCameraGetNodeMap(self.handle, &mut handle));
            Ok(NodeMap{ handle })
        }

        pub fn begin_acquisition(&self) -> Result<(), CameraError> {
            checked_call!(spinCameraBeginAcquisition(self.handle));
            Ok(())
        }

        pub fn end_acquisition(&self) -> Result<(), CameraError> {
            checked_call!(spinCameraEndAcquisition(self.handle));
            Ok(())
        }
    }

    impl Drop for Camera {
        fn drop(&mut self) {
            let _ = self.end_acquisition();
            let _ = self.deinit();
            unsafe { spinCameraRelease(self.handle); }
        }
    }

    pub struct NodeMap {
        handle: *mut std::os::raw::c_void
    }

    impl NodeMap {
        pub fn node(&self, name: &str) -> Result<Node, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinNodeMapGetNode(self.handle, std::ffi::CString::new(name).unwrap().as_c_str().as_ptr(), &mut handle));

            let mut available: bool8_t = False;
            checked_call!(spinNodeIsAvailable(handle, &mut available));
            if available != True {
                return Err(SpinnakerError::Error(format!("node \"{}\" is not available", name)).into());
            }

            Ok(Node{ handle, name: name.to_string(), node_type: RefCell::new(None) })
        }
    }

    pub struct Node {
        handle: *mut std::os::raw::c_void,
        name: String,
        node_type: RefCell<Option<_spinNodeType>>
    }

    impl Node {
        pub fn node_type(&self) -> Result<_spinNodeType, CameraError> {
            if self.node_type.borrow().is_some() {
                Ok(self.node_type.borrow().unwrap())
            } else {
                let mut node_type = std::mem::MaybeUninit::uninit();
                checked_call!(spinNodeGetType(self.handle, node_type.as_mut_ptr()));
                let node_type = unsafe { node_type.assume_init() };
                self.node_type.replace(Some(node_type));
                Ok(node_type)
            }
        }

        pub fn string_value(&self) -> Result<String, CameraError> {
            let mut readable: bool8_t = False;
            checked_call!(spinNodeIsReadable(self.handle, &mut readable));
            if readable != True {
                return Err(SpinnakerError::Error(format!("node \"{}\" is not readable", self.name)).into());
            }

            read_string(self.handle, spinStringGetValue)
        }

        pub fn int_value(&self) -> Result<i64, CameraError> {
            read_value(self.handle, spinIntegerGetValue)
        }

        pub fn set_int_value(&self, value: i64) -> Result<(), CameraError> {
            checked_call!(spinIntegerSetValue(self.handle, value));
            Ok(())
        }

        pub fn float_value(&self) -> Result<f64, CameraError> {
            read_value(self.handle, spinFloatGetValue)
        }

        pub fn set_float_value(&self, value: f64) -> Result<(), CameraError> {
            checked_call!(spinFloatSetValue(self.handle, value));
            Ok(())
        }

        pub fn bool_value(&self) -> Result<bool, CameraError> {
            let result = read_value(self.handle, spinBooleanGetValue)?;
            Ok(result == True)
        }

        pub fn set_bool_value(&self, value: bool) -> Result<(), CameraError> {
            checked_call!(spinBooleanSetValue(self.handle, if value { True } else { False }));
            Ok(())
        }

        pub fn as_string(&self) -> Result<String, CameraError> {
            let mut readable: bool8_t = False;
            checked_call!(spinNodeIsReadable(self.handle, &mut readable));
            if readable != True {
                return Err(SpinnakerError::Error(format!("node \"{}\" is not readable", self.name)).into());
            }

            read_string(self.handle, spinNodeToString)
        }

        pub fn name(&self) -> &str {
            &self.name
        }

        pub fn visibility(&self) -> Result<spinVisibility, CameraError> {
            read_value(self.handle, spinNodeGetVisibility)
        }

        pub fn readable(&self) -> Result<bool, CameraError> {
            let result = read_value(self.handle, spinNodeIsReadable)?;
            Ok(result == True)
        }

        pub fn writable(&self) -> Result<bool, CameraError> {
            let result = read_value(self.handle, spinNodeIsWritable)?;
            Ok(result == True)
        }

        pub fn access_mode(&self) -> Result<ControlAccessMode, CameraError> {
            match (self.readable()?, self.writable()?) {
                (true, true) => Ok(ControlAccessMode::ReadWrite),
                (true, false) => Ok(ControlAccessMode::ReadOnly),
                (false, false) => Ok(ControlAccessMode::None),
                (false, true) => Ok(ControlAccessMode::WriteOnly)
            }
        }

        pub fn display_name(&self) -> Result<String, CameraError> {
            read_string(self.handle, spinNodeGetDisplayName)
        }

        pub fn num_children(&self) -> Result<size_t, CameraError> {
            if self.node_type()? != _spinNodeType_CategoryNode {
                return Ok(0)
            }

            let mut result: size_t = 0;
            checked_call!(spinCategoryGetNumFeatures(self.handle, &mut result));
            Ok(result)
        }

        pub fn child(&self, index: size_t) -> Result<Node, CameraError> {
            let mut handle = std::ptr::null_mut();
            checked_call!(spinCategoryGetFeatureByIndex(self.handle, index, &mut handle));
            let mut child = Node{ handle, name: "".to_string(), node_type: RefCell::new(None) };
            child.name = read_string(child.handle, spinNodeGetName)?;
            Ok(child)
        }

        pub fn current_enum_value_opaque(&self) -> Result<i64, CameraError> {
            let current_entry = read_value(self.handle, spinEnumerationGetCurrentEntry)?;
            let result = read_value(current_entry, spinEnumerationEntryGetIntValue)?;
            checked_call!(spinEnumerationReleaseNode(self.handle, current_entry));
            Ok(result)
        }

        pub fn current_enum_value_actual(&self) -> Result<size_t, CameraError> {
            let current_entry = read_value(self.handle, spinEnumerationGetCurrentEntry)?;
            let result = read_value(current_entry, spinEnumerationEntryGetEnumValue)?;
            checked_call!(spinEnumerationReleaseNode(self.handle, current_entry));
            Ok(result)
        }

        /// Returns enumeration entries and current entry index.
        pub fn enum_entries(&self) -> Result<(Vec<EnumEntry>, usize), CameraError> {
            let mut entries = vec![];
            let mut current_idx = 0;

            let current_value = self.current_enum_value_opaque()?;

            let num_entries = read_value(self.handle, spinEnumerationGetNumEntries)?;
            for i in 0..num_entries {
                let mut entry_node = std::mem::MaybeUninit::uninit();
                checked_call!(spinEnumerationGetEntryByIndex(self.handle, i, entry_node.as_mut_ptr()));
                let entry_node = unsafe { entry_node.assume_init() };

                let entry_is_available = read_value(entry_node, spinNodeIsAvailable)?;
                if entry_is_available != True { continue; }

                let opaque_value = read_value(entry_node, spinEnumerationEntryGetIntValue)?;
                let actual_value = read_value(entry_node, spinEnumerationEntryGetEnumValue).ok();
                let entry_symbolic_value = read_string(entry_node, spinEnumerationEntryGetSymbolic)?;

                entries.push(EnumEntry{
                    opaque: opaque_value,
                    actual: actual_value,
                    symbolic: entry_symbolic_value
                });
                if opaque_value == current_value {
                    current_idx = entries.len() - 1;
                }

                checked_call!(spinEnumerationReleaseNode(self.handle, entry_node));
            }

            Ok((entries, current_idx))
        }

        pub fn set_enum_entry(&mut self, enum_value: i64) -> Result<(), CameraError> {
            checked_call!(spinEnumerationSetIntValue(self.handle, enum_value));
            Ok(())
        }

        pub fn min_float(&self) -> Result<f64, CameraError> {
            read_value(self.handle, spinFloatGetMin)
        }

        pub fn max_float(&self) -> Result<f64, CameraError> {
            read_value(self.handle, spinFloatGetMax)
        }

        pub fn min_int(&self) -> Result<i64, CameraError> {
            read_value(self.handle, spinIntegerGetMin)
        }

        pub fn max_int(&self) -> Result<i64, CameraError> {
            read_value(self.handle, spinIntegerGetMax)
        }

        pub fn int_increment(&self) -> Result<i64, CameraError> {
            read_value(self.handle, spinIntegerGetInc)
        }
    }
}

fn to_pix_format(pix_format: _spinPixelFormatEnums) -> Result<ga_image::PixelFormat, CameraError> {
    match pix_format {
        _spinPixelFormatEnums_PixelFormat_Mono8 => Ok(ga_image::PixelFormat::Mono8),
        _spinPixelFormatEnums_PixelFormat_Mono16 => Ok(ga_image::PixelFormat::Mono16),
        _spinPixelFormatEnums_PixelFormat_BayerGR8 => Ok(ga_image::PixelFormat::CfaGRBG8),
        _spinPixelFormatEnums_PixelFormat_BayerRG8 => Ok(ga_image::PixelFormat::CfaRGGB8),
        _spinPixelFormatEnums_PixelFormat_BayerGB8 => Ok(ga_image::PixelFormat::CfaGBRG8),
        _spinPixelFormatEnums_PixelFormat_BayerBG8 => Ok(ga_image::PixelFormat::CfaBGGR8),
        _spinPixelFormatEnums_PixelFormat_BayerGR16 => Ok(ga_image::PixelFormat::CfaGRBG16),
        _spinPixelFormatEnums_PixelFormat_BayerRG16 => Ok(ga_image::PixelFormat::CfaRGGB16),
        _spinPixelFormatEnums_PixelFormat_BayerGB16 => Ok(ga_image::PixelFormat::CfaGBRG16),
        _spinPixelFormatEnums_PixelFormat_BayerBG16 => Ok(ga_image::PixelFormat::CfaBGGR16),

        _ => Err(SpinnakerError::Error(format!("unsupported Spinnaker pixel format: {}", pix_format)).into())
    }
}

pub struct SpinnakerDriver {
    system: spin::System,
    cameras: Option<spin::CameraList>
}

impl SpinnakerDriver {
    pub fn new() -> Result<SpinnakerDriver, CameraError> {
        let mut system = spin::System::new()?;

        let mut lib_ver = std::mem::MaybeUninit::uninit();
        checked_call!(spinSystemGetLibraryVersion(system.get(), lib_ver.as_mut_ptr()));
        let lib_ver = unsafe { lib_ver.assume_init() };
        println!("Spinnaker version: {}.{}.{}.{}", lib_ver.major, lib_ver.minor, lib_ver.type_, lib_ver.build);

        Ok(SpinnakerDriver{ system, cameras: None })
    }
}

impl Driver for SpinnakerDriver {
    fn name(&self) -> &'static str { "Spinnaker" }

    fn enumerate_cameras(&mut self) -> Result<Vec<CameraInfo>, CameraError> {
        let mut camera_list = spin::CameraList::new()?;
        checked_call!(spinSystemGetCameras(self.system.get(), camera_list.get()));

        let mut result = vec![];

        for i in 0..camera_list.num_cameras()? {
            let camera = camera_list.camera(i)?;
            let node_map = camera.device_node_map()?;
            result.push(CameraInfo{
                id: i.into(),
                name: format!("{}", node_map.node(genicam::DEVICE_MODEL_NAME)?.string_value()?)
            });
        }

        self.cameras = Some(camera_list);

        Ok(result)
    }

    /// Returns camera with acquisition enabled.
    fn open_camera(&mut self, id: CameraId) -> Result<Box<dyn Camera>, CameraError> {
        let index: usize = id.into();
        if self.cameras.is_none() || index > self.cameras.as_ref().unwrap().num_cameras()? {
            return Err(SpinnakerError::Error(format!("invalid camera id: {}", index)).into());
        }

        let mut camera_handle = self.cameras.as_ref().unwrap().camera(index)?;
        camera_handle.init()?;
        camera_handle.begin_acquisition()?;

        let genicam_node_map = camera_handle.genicam_node_map()?;
        let temperature_node = genicam_node_map.node(genicam::DEVICE_TEMPERATURE).ok();

        to_pix_format(genicam_node_map.node(genicam::PIXEL_FORMAT)?.current_enum_value_actual()? as _spinPixelFormatEnums)?;

        Ok(Box::new(SpinnakerCamera{
            id,
            name: genicam_node_map.node(genicam::DEVICE_MODEL_NAME)?.string_value()?,
            camera_handle: Arc::new(camera_handle),
            temperature_node,
            controls: vec![],
            roi_offset: (0, 0)
        }))
    }
}

struct ControlData {
    node: spin::Node,
    enum_entries: Option<Vec<spin::EnumEntry>>
}

struct SpinnakerCamera {
    id: CameraId,
    name: String,
    camera_handle: Arc<spin::Camera>,
    temperature_node: Option<spin::Node>,
    controls: Vec<ControlData>,
    roi_offset: (u32, u32)
}

impl SpinnakerCamera {
    fn add_num_control(
        controls: &mut Vec<CameraControl>,
        control_data: &mut Vec<ControlData>,
        node_map: &spin::NodeMap,
        name: &str,
        display_name: &str,
        step: f64,
        num_decimals: usize,
        is_exposure_time: bool
    ) -> Result<(), CameraError> {
        match node_map.node(name).ok() {
            Some(node) => {
                controls.push(CameraControl::Number(NumberControl{
                    base: CameraControlBase{
                        id: CameraControlId(control_data.len() as u64),
                        label: display_name.to_string(),
                        refreshable: name == genicam::EXPOSURE_TIME || name == genicam::GAIN,
                        access_mode: node.access_mode()?,
                        auto_state: None,
                        on_off_state: None,
                        requires_capture_pause: false
                    },
                    value: node.float_value()?,
                    min: node.min_float()?,
                    max: node.max_float()?,
                    step,
                    num_decimals,
                    is_exposure_time
                }));

                control_data.push(ControlData{ node, enum_entries: None });
            },
            _ => ()
        }

        Ok(())
    }

    fn add_list_control(
        controls: &mut Vec<CameraControl>,
        control_data: &mut Vec<ControlData>,
        node_map: &spin::NodeMap,
        name: &str,
        display_name: &str,
        requires_capture_pause: bool,
        force_access_mode: Option<ControlAccessMode>
    ) -> Result<(), CameraError> {
        match node_map.node(name).ok() {
            Some(node) => {
                let enum_entries = node.enum_entries()?;

                controls.push(CameraControl::List(ListControl{
                    base: CameraControlBase{
                        id: CameraControlId(control_data.len() as u64),
                        label: display_name.to_string(),
                        refreshable: false,
                        access_mode: if let Some(mode) = force_access_mode { mode } else { node.access_mode()? },
                        auto_state: None,
                        on_off_state: None,
                        requires_capture_pause
                    },
                    items: enum_entries.0.iter().map(|ee| ee.symbolic().to_string()).collect(),
                    current_idx: enum_entries.1
                }));

                control_data.push(ControlData{ node, enum_entries: Some(enum_entries.0) });
            },
            _ => ()
        }

        Ok(())
    }

    fn add_bool_control(
        controls: &mut Vec<CameraControl>,
        control_data: &mut Vec<ControlData>,
        node_map: &spin::NodeMap,
        name: &str,
        display_name: &str,
        requires_capture_pause: bool
    ) -> Result<(), CameraError> {
        match node_map.node(name).ok() {
            Some(node) => {
                controls.push(CameraControl::Boolean(BooleanControl{
                    base: CameraControlBase{
                        id: CameraControlId(control_data.len() as u64),
                        label: display_name.to_string(),
                        refreshable: false,
                        access_mode: node.access_mode()?,
                        auto_state: None,
                        on_off_state: None,
                        requires_capture_pause
                    },
                    state: node.bool_value()?
                }));

                control_data.push(ControlData{ node, enum_entries: None });
            },
            _ => ()
        }

        Ok(())
    }
}

impl Camera for SpinnakerCamera {
    fn id(&self) -> CameraId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn enumerate_controls(&mut self) -> Result<Vec<CameraControl>, CameraError> {
        let mut control_data = vec![];
        let mut controls = vec![];

        let genicam_node_map = self.camera_handle.genicam_node_map()?;

        // for now just add basic controls explicitly ------------------------------

        SpinnakerCamera::add_list_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            genicam::PIXEL_FORMAT,
            "Pixel Format",
            true,
            // need to force this one; it reports as unwritable when acquisition is enabled
            Some(ControlAccessMode::WriteOnly)
        )?;

        SpinnakerCamera::add_num_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            genicam::EXPOSURE_TIME,
            "Exposure Time (µs)",
            1.0,
            0,
            true
        )?;

        SpinnakerCamera::add_list_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            genicam::EXPOSURE_AUTO,
            "Exposure Auto",
            false,
            None
        )?;

        //TODO: use `spinFloatGetUnit` in case control has device-specific units
        SpinnakerCamera::add_num_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            genicam::GAIN,
            "Gain",
            0.01,
            2,
            false
        )?;

        SpinnakerCamera::add_list_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            genicam::GAIN_AUTO,
            "Gain Auto",
            false,
            None
        )?;

        SpinnakerCamera::add_num_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            genicam::GAMMA,
            "Gamma",
            0.1,
            2,
            false
        )?;

        SpinnakerCamera::add_bool_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            flir::GAMMA_ENABLE,
            "Gamma Enabled",
            false
        )?;

        SpinnakerCamera::add_bool_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            flir::GAMMA_ENABLED,
            "Gamma Enabled",
            false
        )?;

        SpinnakerCamera::add_num_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            genicam::ACQUISITION_FRAME_RATE,
            "Frame Rate",
            1.0,
            0,
            false
        )?;

        SpinnakerCamera::add_list_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            flir::ACQUISITION_FRAME_RATE_AUTO,
            "Frame Rate Auto",
            false,
            None
        )?;

        SpinnakerCamera::add_bool_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            flir::ACQUISITION_FRAME_RATE_ENABLED,
            "Frame Rate Enabled",
            false
        )?;

        SpinnakerCamera::add_bool_control(
            &mut controls,
            &mut control_data,
            &genicam_node_map,
            flir::ACQUISITION_FRAME_RATE_ENABLE,
            "Frame Rate Enabled",
            false
        )?;

        //TODO: add automatic reading of unknown controls, including those from the "device" and "stream" maps,
        // make it configurable via config file and/or via GUI

        self.controls = control_data;

        Ok(controls)
    }

    fn create_capturer(&self) -> Result<Box<dyn FrameCapturer + Send>, CameraError> {
        // The returned frame capturer will share `camera_handle` (via `Arc`). Spinnaker allows using its functions
        // from multiple threads without additional synchronization. The `SpinnakerCamera`'s instance will be used
        // by the main thread, and the `FrameCapturer`'s instance - by the capture thread.
        Ok(Box::new(SpinnakerFrameCapturer{
            camera_handle: self.camera_handle.clone(),
        }))
    }

    fn set_number_control(&self, id: CameraControlId, value: f64) -> Result<(), CameraError> {
        self.controls[id.0 as usize].node.set_float_value(value)?;
        Ok(())
    }

    fn set_list_control(&mut self, id: CameraControlId, option_idx: usize) -> Result<(), CameraError> {
        let control_data = &mut self.controls[id.0 as usize];

        if control_data.node.name() == genicam::PIXEL_FORMAT {
            let chosen_pix_fmt =
                control_data.enum_entries.as_ref().unwrap()[option_idx].actual().unwrap() as _spinPixelFormatEnums;

            // return error if we do not support the chosen format
            to_pix_format(chosen_pix_fmt)?;
        }

        control_data.node.set_enum_entry(control_data.enum_entries.as_ref().unwrap()[option_idx].opaque())?;
        Ok(())
    }

    fn set_auto(&mut self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        unimplemented!()
    }

    fn set_on_off(&self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        unimplemented!()
    }

    fn get_number_control(&self, id: CameraControlId) -> Result<f64, CameraError> {
        self.controls[id.0 as usize].node.float_value()
    }

    fn get_list_control(&self, id: CameraControlId) -> Result<usize, CameraError> {
        let control_data = &self.controls[id.0 as usize];
        let current = control_data.node.current_enum_value_opaque()?;
        for (idx, entry) in control_data.enum_entries.as_ref().unwrap().iter().enumerate() {
            if entry.opaque() == current {
                return Ok(idx);
            }
        }

        panic!("Unexpected current enumeration entry.");
    }

    fn temperature(&self) -> Option<f64> {
        match &self.temperature_node {
            None => None,
            Some(node) => node.float_value().ok()
        }
    }

    fn set_roi(&mut self, x0: u32, y0: u32, width: u32, height: u32) -> Result<(), CameraError> {
        let node_map = self.camera_handle.genicam_node_map()?;

        let max_width = node_map.node(genicam::WIDTH_MAX)?.int_value()? as u32;
        let max_height = node_map.node(genicam::HEIGHT_MAX)?.int_value()? as u32;

        let granularity_x = node_map.node(genicam::WIDTH)?.int_increment()?;
        let granularity_y = node_map.node(genicam::HEIGHT)?.int_increment()?;

        // rounds `x` down to the closest multiple of `n`
        macro_rules! downmult { ($x:expr, $n:expr) => { ($x) / ($n) * ($n) } }

        node_map.node(genicam::WIDTH)?.set_int_value(downmult!(width.min(max_width) as i64, granularity_x))?;
        node_map.node(genicam::HEIGHT)?.set_int_value(downmult!(height.min(max_height) as i64, granularity_y))?;
        node_map.node(genicam::OFFSET_X)?.set_int_value(downmult!((self.roi_offset.0 + x0) as i64, granularity_x))?;
        node_map.node(genicam::OFFSET_Y)?.set_int_value(downmult!((self.roi_offset.1 + y0) as i64, granularity_y))?;

        self.roi_offset = (self.roi_offset.0 + x0, self.roi_offset.1 + y0);

        Ok(())
    }

    fn unset_roi(&mut self) -> Result<(), CameraError> {
        let node_map = self.camera_handle.genicam_node_map()?;

        let max_width = node_map.node(genicam::WIDTH_MAX)?.int_value()?;
        let max_height = node_map.node(genicam::HEIGHT_MAX)?.int_value()?;

        node_map.node(genicam::OFFSET_X)?.set_int_value(0)?;
        node_map.node(genicam::OFFSET_Y)?.set_int_value(0)?;
        node_map.node(genicam::WIDTH)?.set_int_value(max_width)?;
        node_map.node(genicam::HEIGHT)?.set_int_value(max_height)?;

        self.roi_offset = (0, 0);

        Ok(())
    }

    fn set_boolean_control(&mut self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        self.controls[id.0 as usize].node.set_bool_value(state)?;
        Ok(())
    }

    fn get_boolean_control(&self, id: CameraControlId) -> Result<bool, CameraError> {
        self.controls[id.0 as usize].node.bool_value()
    }
}

struct SpinnakerFrameCapturer {
    camera_handle: Arc<spin::Camera>
}

unsafe impl Send for SpinnakerFrameCapturer {}

impl FrameCapturer for SpinnakerFrameCapturer {
    fn capture_frame(&mut self, dest_image: &mut Image) -> Result<(), CameraError> {
        let result = self.camera_handle.next_image();
        if let Err(CameraError::SpinnakerError(SpinnakerError::Internal(spin_err))) = result {
            if spin_err == _spinError_SPINNAKER_ERR_TIMEOUT {
                println!("WARNING: Timeout when waiting for new frame.");
                return Err(CameraError::FrameUnavailable);
            }
        }

        let frame = result?;
        let status = frame.status()?;
        match status {
            _spinImageStatus_IMAGE_NO_ERROR => (),

            _spinImageStatus_IMAGE_MISSING_PACKETS
            | _spinImageStatus_IMAGE_DATA_INCOMPLETE
            => {
                println!("WARNING: Captured corrupted image. Error code: {}.", status)
            },

            _spinImageStatus_IMAGE_INFO_INCONSISTENT => {
                println!("WARNING: Captured frame with inconsistent info, ignoring.");
                return Err(CameraError::FrameUnavailable);
            }

            _ => return Err(SpinnakerError::Error(
                format!("error {} ({})", status, spin::read_string(status, spinImageGetStatusDescription)?)
            ).into())
        }

        let f_width = frame.width()? as u32;
        let f_height = frame.height()? as u32;
        let f_stride = frame.stride()? as usize;

        // Happens sometimes under Linux with a GigE camera (packet loss? Corruption?).
        if frame.pixel_format()? == _spinPixelFormatEnums_UNKNOWN_PIXELFORMAT {
            println!("WARNING: Captured frame with unspecified pixel format, ignoring.");
            return Err(CameraError::FrameUnavailable);
        }

        let f_pix_fmt = to_pix_format(frame.pixel_format()?)?;

        let frame_pixels: &[u8] = unsafe { std::slice::from_raw_parts(
            frame.data_ptr()? as *const u8,
            frame.data_size()? as usize
        ) };

        if dest_image.bytes_per_line() != f_stride ||
            dest_image.width() != f_width ||
            dest_image.height() != f_height ||
            dest_image.pixel_format() != f_pix_fmt {

            // there may be some padding at the end of `frame_pixels` which we do not need
            let num_bytes_to_use = f_height as usize * f_stride as usize * f_pix_fmt.bytes_per_pixel();
            let frame_pixels = &frame_pixels[..num_bytes_to_use];

            *dest_image = Image::new_from_pixels(
                f_width,
                f_height,
                Some(f_stride),
                f_pix_fmt,
                None,
                frame_pixels.to_vec()
            );
        } else {
            // there may be some padding at the end of `frame_pixels` which we do not need
            let num_bytes_to_use = f_height as usize * f_stride as usize * f_pix_fmt.bytes_per_pixel();
            dest_image.raw_pixels_mut().copy_from_slice(&frame_pixels[..num_bytes_to_use]);
        }

        // TODO - check endianess for >8 bpp pixel formats

        Ok(())
    }

    fn pause(&mut self) -> Result<(), CameraError> {
        self.camera_handle.end_acquisition()
    }

    fn resume(&mut self) -> Result<(), CameraError> {
        self.camera_handle.begin_acquisition()
    }
}
