
use crate::{
    BackendSpecificError, BuildStreamError, Data, DefaultStreamConfigError, DeviceNameError,
    DevicesError, PauseStreamError, PlayStreamError, SampleFormat, StreamConfig, StreamError,
    SupportedStreamConfig, SupportedStreamConfigRange, SupportedStreamConfigsError,
};
use traits::{DeviceTrait, HostTrait, StreamTrait};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::AudioContext;

use std::cell::RefCell;
use std::rc::Rc;

// The wasm backend currently works by instantiating an `AudioContext` object per `Stream`.
// Creating a stream creates a new `AudioContext`. Destroying a stream destroys it.

/// The default wasm host type.
#[derive(Debug)]
pub struct Host;

/// Content is false if the iterator is empty.
pub struct Devices(bool);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device;

#[derive(Clone)]
pub struct Stream {
    // A reference to an `AudioContext` object.
    audio_ctxt_ref: Rc<RefCell<AudioContext>>,
}

// Index within the `streams` array of the events loop.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamId(usize);

pub type SupportedInputConfigs = ::std::vec::IntoIter<SupportedStreamConfigRange>;
pub type SupportedOutputConfigs = ::std::vec::IntoIter<SupportedStreamConfigRange>;

impl Host {
    pub fn new() -> Result<Self, crate::HostUnavailable> {
        Ok(Host)
    }
}

impl Devices {
    fn new() -> Result<Self, DevicesError> {
        Ok(Self::default())
    }
}

const DEFAULT_SAMPLE_RATE: u32 = 48000; // TODO replace hard coded value

impl Device {
    #[inline]
    fn name(&self) -> Result<String, DeviceNameError> {
        Ok("Default Device".to_owned())
    }

    #[inline]
    fn supported_input_configs(
        &self,
    ) -> Result<SupportedInputConfigs, SupportedStreamConfigsError> {
        // unimplemented!();
        Err(SupportedStreamConfigsError::BackendSpecific {
            err: BackendSpecificError {
                description: "unimplemented!".to_string(),
            },
        })
    }

    #[inline]
    fn supported_output_configs(
        &self,
    ) -> Result<SupportedOutputConfigs, SupportedStreamConfigsError> {
        // TODO: right now cpal's API doesn't allow flexibility here
        //       "48000" and "2" (channels) have also been hard-coded in the rest of the code ; if
        //       this ever becomes more flexible, don't forget to change that
        //       According to https://developer.mozilla.org/en-US/docs/Web/API/BaseAudioContext/createBuffer
        //       browsers must support 1 to 32 channels at leats and 8,000 Hz to 96,000 Hz.
        //
        //       UPDATE: We can do this now. Might be best to use `crate::COMMON_SAMPLE_RATES` and
        //       filter out those that lay outside the range specified above.
        Ok(vec![SupportedStreamConfigRange {
            channels: 2,
            min_sample_rate: ::SampleRate(DEFAULT_SAMPLE_RATE),
            max_sample_rate: ::SampleRate(DEFAULT_SAMPLE_RATE),
            sample_format: ::SampleFormat::F32,
        }]
        .into_iter())
    }

    fn default_input_config(&self) -> Result<SupportedStreamConfig, DefaultStreamConfigError> {
        // unimplemented!();
        Err(DefaultStreamConfigError::StreamTypeNotSupported)
    }

    fn default_output_config(&self) -> Result<SupportedStreamConfig, DefaultStreamConfigError> {
        // TODO: because it is hard coded, see supported_output_configs.
        Ok(SupportedStreamConfig {
            channels: 2,
            sample_rate: ::SampleRate(DEFAULT_SAMPLE_RATE),
            sample_format: ::SampleFormat::F32,
        })
    }
}

impl HostTrait for Host {
    type Devices = Devices;
    type Device = Device;

    fn is_available() -> bool {
        // Assume this host is always available on wasm.
        true
    }

    fn devices(&self) -> Result<Self::Devices, DevicesError> {
        Devices::new()
    }

    fn default_input_device(&self) -> Option<Self::Device> {
        default_input_device()
    }

    fn default_output_device(&self) -> Option<Self::Device> {
        default_output_device()
    }
}

impl DeviceTrait for Device {
    type SupportedInputConfigs = SupportedInputConfigs;
    type SupportedOutputConfigs = SupportedOutputConfigs;
    type Stream = Stream;

    fn name(&self) -> Result<String, DeviceNameError> {
        Device::name(self)
    }

    fn supported_input_configs(
        &self,
    ) -> Result<Self::SupportedInputConfigs, SupportedStreamConfigsError> {
        Device::supported_input_configs(self)
    }

    fn supported_output_configs(
        &self,
    ) -> Result<Self::SupportedOutputConfigs, SupportedStreamConfigsError> {
        Device::supported_output_configs(self)
    }

    fn default_input_config(&self) -> Result<SupportedStreamConfig, DefaultStreamConfigError> {
        Device::default_input_config(self)
    }

    fn default_output_config(&self) -> Result<SupportedStreamConfig, DefaultStreamConfigError> {
        Device::default_output_config(self)
    }

    fn build_input_stream_raw<D, E>(
        &self,
        _config: &StreamConfig,
        _sample_format: SampleFormat,
        _data_callback: D,
        _error_callback: E,
    ) -> Result<Self::Stream, BuildStreamError>
    where
        D: FnMut(&Data) + Send + 'static,
        E: FnMut(StreamError) + Send + 'static,
    {
        // unimplemented!()
        Err(BuildStreamError::StreamConfigNotSupported)
    }

    fn build_output_stream_raw<D, E>(
        &self,
        _config: &StreamConfig,
        sample_format: SampleFormat,
        mut data_callback: D,
        _error_callback: E,
    ) -> Result<Self::Stream, BuildStreamError>
    where
        D: FnMut(&mut Data) + Send + 'static,
        E: FnMut(StreamError) + Send + 'static,
    {
        assert_eq!(
            sample_format,
            SampleFormat::F32,
            "wasm32 backend currently only supports `f32` data",
        );

        // Create the stream.
        let audio_ctxt_ref = Rc::new(RefCell::new(AudioContext::new().unwrap()));
        let stream = Stream { audio_ctxt_ref };

        let stream_clone = stream.clone();

        let callback = Closure::wrap(Box::new(move || {
            let audio_ctxt = stream_clone.audio_ctxt_ref.borrow();

            let mut temporary_buffer : Vec<f32> = vec![0.0; DEFAULT_SAMPLE_RATE as usize * 2 / 3];
            {
                let len = temporary_buffer.len();
                let data = temporary_buffer.as_mut_ptr() as *mut ();
                let sample_format = SampleFormat::F32;
                let mut data = unsafe { Data::from_parts(data, len, sample_format) };
                data_callback(&mut data);
            }

            let num_channels: usize = 2; // TODO: hard coded value
            debug_assert_eq!(temporary_buffer.len() % num_channels, 0);

            let buf_len = temporary_buffer.len();
            let buffer = audio_ctxt
                .create_buffer(
                    num_channels as u32,
                    (buf_len / num_channels) as u32,
                    DEFAULT_SAMPLE_RATE as f32,
                )
                .unwrap();
            for channel in 0..num_channels {
                let mut buffer_content : Vec<f32> = buffer.get_channel_data(channel as u32).unwrap();
                for i in 0..buffer_content.len() {
                    buffer_content[i] = temporary_buffer[i * num_channels + channel];
                }
            }

            let node = audio_ctxt.create_buffer_source().unwrap();
            node.set_buffer(Some(&buffer));
            node.connect_with_audio_node(&audio_ctxt.destination())
                .unwrap();
            let _start_result = node.start().unwrap();
        }) as Box<dyn FnMut()>);

        let _interval_id = web_sys::window()
            .unwrap()
            .set_interval_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                10,
            ).unwrap();

        callback.forget();

        Ok(stream)
    }
}

impl StreamTrait for Stream {
    fn play(&self) -> Result<(), PlayStreamError> {
        let _resume_result = self.audio_ctxt_ref.borrow().resume();
        Ok(())
    }

    fn pause(&self) -> Result<(), PauseStreamError> {
        let _suspend_result = self.audio_ctxt_ref.borrow().suspend();
        Ok(())
    }
}

impl Default for Devices {
    fn default() -> Devices {
        // We produce an empty iterator if the WebAudio API isn't available.
        Devices(is_webaudio_available())
    }
}
impl Iterator for Devices {
    type Item = Device;
    #[inline]
    fn next(&mut self) -> Option<Device> {
        if self.0 {
            self.0 = false;
            Some(Device)
        } else {
            None
        }
    }
}

#[inline]
fn default_input_device() -> Option<Device> {
    // unimplemented!();
    None
}

#[inline]
fn default_output_device() -> Option<Device> {
    if is_webaudio_available() {
        Some(Device)
    } else {
        None
    }
}

// Detects whether the global `Window` is available.
fn is_webaudio_available() -> bool {
    web_sys::window().is_some()
}
