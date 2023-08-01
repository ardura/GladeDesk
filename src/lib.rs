#![allow(non_snake_case)]
mod ui_knob;
mod db_meter;
use atomic_float::AtomicF32;
use nih_plug::{prelude::*};
use nih_plug_egui::{create_egui_editor, egui::{self, Color32, Rect, Rounding, RichText, FontId, Pos2}, EguiState, widgets};
use std::{sync::{Arc}, ops::RangeInclusive, collections::VecDeque};

/***************************************************************************
 * Glade Desk by Ardura
 * 
 * Build with: cargo xtask bundle GladeDesk --profile <release or profiling>
 * *************************************************************************/

 // GUI Colors
const A_KNOB_OUTSIDE_COLOR: Color32 = Color32::from_rgb(112,141,129);
const A_BACKGROUND_COLOR: Color32 = Color32::from_rgb(0,20,39);
const A_KNOB_INSIDE_COLOR: Color32 = Color32::from_rgb(244,213,141);
const A_KNOB_OUTSIDE_COLOR2: Color32 = Color32::from_rgb(242,100,25);

// Plugin sizing
const WIDTH: u32 = 500;
const HEIGHT: u32 = 600;

/// The time it takes for the peak meter to decay by 12 dB after switching to complete silence.
const PEAK_METER_DECAY_MS: f64 = 100.0;

pub struct Gain {
    params: Arc<GainParams>,

    // normalize the peak meter's response based on the sample rate with this
    out_meter_decay_weight: f32,

    // Buffers
    left_vec: VecDeque<f32>,
    right_vec: VecDeque<f32>,

    // The current data for the different meters
    out_meter: Arc<AtomicF32>,
    in_meter: Arc<AtomicF32>,
}

#[derive(Params)]
struct GainParams {
    /// The editor state, saved together with the parameter state so the custom scaling can be
    /// restored.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "free_gain"]
    pub free_gain: FloatParam,

    #[id = "Push"]
    pub push_amount: FloatParam,

    #[id = "1_Coeff"]
    pub slider_1_coeff: FloatParam,

    #[id = "1_Skew"]
    pub slider_1_skew: FloatParam,

    #[id = "2_Coeff"]
    pub slider_2_coeff: FloatParam,

    #[id = "2_Skew"]
    pub slider_2_skew: FloatParam,

    #[id = "output_gain"]
    pub output_gain: FloatParam,

    #[id = "dry_wet"]
    pub dry_wet: FloatParam,
}

impl Default for Gain {
    fn default() -> Self {
        Self {
            params: Arc::new(GainParams::default()),
            out_meter_decay_weight: 1.0,
            out_meter: Arc::new(AtomicF32::new(util::MINUS_INFINITY_DB)),
            in_meter: Arc::new(AtomicF32::new(util::MINUS_INFINITY_DB)),
            left_vec: VecDeque::from(vec![0.0,0.0]),
            right_vec: VecDeque::from(vec![0.0,0.0]),
        }
    }
}

impl Default for GainParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(WIDTH, HEIGHT),

            // Input gain dB parameter (free as in unrestricted nums)
            free_gain: FloatParam::new(
                "Input Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-12.0),
                    max: util::db_to_gain(12.0),
                    factor: FloatRange::gain_skew_factor(-12.0, 12.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(30.0))
            .with_unit(" In Gain")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            // push_amount Parameter
            push_amount: FloatParam::new(
                "Push",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(2)),

            // Coeff parameter
            slider_1_coeff: FloatParam::new(
                "1",
                0.0,
                FloatRange::Linear { min: -1.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(6)),

            // Skew parameter
            slider_1_skew: FloatParam::new(
                "",
                0.0,
                FloatRange::Linear { min: -1.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(6)),

            // Coeff parameter
            slider_2_coeff: FloatParam::new(
                "2",
                0.0,
                FloatRange::Linear { min: -1.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(6)),

            // Skew parameter
            slider_2_skew: FloatParam::new(
                "",
                0.0,
                FloatRange::Linear { min: -1.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(6)),

            // Output gain parameter
            output_gain: FloatParam::new(
                "Output Gain",
                util::db_to_gain(-2.8),
                FloatRange::Skewed {
                    min: util::db_to_gain(-12.0),
                    max: util::db_to_gain(12.0),
                    factor: FloatRange::gain_skew_factor(-12.0, 12.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" Out Gain")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            // Dry/Wet parameter
            dry_wet: FloatParam::new(
                "Dry/Wet",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_unit("% Wet")
            .with_value_to_string(formatters::v2s_f32_percentage(2))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
}

impl Plugin for Gain {
    const NAME: &'static str = "Glade Desk";
    const VENDOR: &'static str = "Ardura";
    const URL: &'static str = "https://github.com/ardura";
    const EMAIL: &'static str = "azviscarra@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // This looks like it's flexible for running the plugin in mono or stereo
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {main_input_channels: NonZeroU32::new(2), main_output_channels: NonZeroU32::new(2), ..AudioIOLayout::const_default()},
        AudioIOLayout {main_input_channels: NonZeroU32::new(1), main_output_channels: NonZeroU32::new(1), ..AudioIOLayout::const_default()},
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(self: &Gain, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let in_meter = self.in_meter.clone();
        let out_meter = self.out_meter.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                egui::CentralPanel::default()
                    .show(egui_ctx, |ui| {
                        // Change colors - there's probably a better way to do this
                        let style_var = ui.style_mut().clone();

                        // Trying to draw background as rect
                        ui.painter().rect_filled(
                            Rect::from_x_y_ranges(
                                RangeInclusive::new(0.0, WIDTH as f32), 
                                RangeInclusive::new(0.0, HEIGHT as f32)), 
                            Rounding::from(16.0), A_BACKGROUND_COLOR);

                        // Screws for that vintage look
                        let screw_space = 10.0;
                        ui.painter().circle_filled(Pos2::new(screw_space,screw_space), 4.0, Color32::DARK_GRAY);
                        ui.painter().circle_filled(Pos2::new(screw_space,HEIGHT as f32 - screw_space), 4.0, Color32::DARK_GRAY);
                        ui.painter().circle_filled(Pos2::new(WIDTH as f32 - screw_space,screw_space), 4.0, Color32::DARK_GRAY);
                        ui.painter().circle_filled(Pos2::new(WIDTH as f32 - screw_space,HEIGHT as f32 - screw_space), 4.0, Color32::DARK_GRAY);

                        ui.set_style(style_var);

                        // GUI Structure
                        ui.vertical(|ui| {
                            // Spacing :)
                            ui.label(RichText::new("    Glade Desk").font(FontId::proportional(14.0)).color(A_KNOB_OUTSIDE_COLOR)).on_hover_text("by Ardura!");

                            // Peak Meters
                            let in_meter = util::gain_to_db(in_meter.load(std::sync::atomic::Ordering::Relaxed));
                            let in_meter_text = if in_meter > util::MINUS_INFINITY_DB {
                                format!("{in_meter:.1} dBFS Input")
                            } else {
                                String::from("-inf dBFS Input")
                            };
                            let in_meter_normalized = (in_meter + 60.0) / 60.0;
                            ui.allocate_space(egui::Vec2::splat(2.0));
                            let mut in_meter_obj = db_meter::DBMeter::new(in_meter_normalized).text(in_meter_text);
                            in_meter_obj.set_background_color(A_KNOB_OUTSIDE_COLOR);
                            in_meter_obj.set_bar_color(A_KNOB_INSIDE_COLOR);
                            in_meter_obj.set_border_color(Color32::BLACK);
                            ui.add(in_meter_obj);

                            let out_meter = util::gain_to_db(out_meter.load(std::sync::atomic::Ordering::Relaxed));
                            let out_meter_text = if out_meter > util::MINUS_INFINITY_DB {
                                format!("{out_meter:.1} dBFS Output")
                            } else {
                                String::from("-inf dBFS Output")
                            };
                            let out_meter_normalized = (out_meter + 60.0) / 60.0;
                            ui.allocate_space(egui::Vec2::splat(2.0));
                            let mut out_meter_obj = db_meter::DBMeter::new(out_meter_normalized).text(out_meter_text);
                            out_meter_obj.set_background_color(A_KNOB_OUTSIDE_COLOR);
                            out_meter_obj.set_bar_color(A_KNOB_INSIDE_COLOR);
                            out_meter_obj.set_border_color(Color32::BLACK);
                            ui.add(out_meter_obj);

                            ui.horizontal(|ui| {
                                let knob_size = 40.0;
                                ui.vertical(|ui| {
                                    let mut gain_knob = ui_knob::ArcKnob::for_param(&params.free_gain, setter, knob_size);
                                    gain_knob.preset_style(ui_knob::KnobStyle::SmallTogether);
                                    gain_knob.set_fill_color(A_KNOB_OUTSIDE_COLOR2);
                                    gain_knob.set_line_color(A_KNOB_OUTSIDE_COLOR);
                                    ui.add(gain_knob);

                                    let mut push_knob = ui_knob::ArcKnob::for_param(&params.push_amount, setter, knob_size);
                                    push_knob.preset_style(ui_knob::KnobStyle::SmallTogether);
                                    push_knob.set_fill_color(A_KNOB_OUTSIDE_COLOR2);
                                    push_knob.set_line_color(A_KNOB_OUTSIDE_COLOR);
                                    ui.add(push_knob);
                                });

                                ui.vertical(|ui | {
                                    let mut output_knob = ui_knob::ArcKnob::for_param(&params.output_gain, setter, knob_size);
                                    output_knob.preset_style(ui_knob::KnobStyle::SmallTogether);
                                    output_knob.set_fill_color(A_KNOB_OUTSIDE_COLOR2);
                                    output_knob.set_line_color(A_KNOB_OUTSIDE_COLOR);
                                    ui.add(output_knob);
                                
                                    let mut dry_wet_knob = ui_knob::ArcKnob::for_param(&params.dry_wet, setter, knob_size);
                                    dry_wet_knob.preset_style(ui_knob::KnobStyle::SmallTogether);
                                    dry_wet_knob.set_fill_color(A_KNOB_OUTSIDE_COLOR2);
                                    dry_wet_knob.set_line_color(A_KNOB_OUTSIDE_COLOR);
                                    ui.add(dry_wet_knob);
                                });
                            });
                            
                            //sliders
                            ui.vertical(|ui| {
                                ui.horizontal(|ui|{
                                    ui.add(widgets::ParamSlider::for_param(&params.slider_1_coeff, setter).with_width(200.0));
                                    ui.add(widgets::ParamSlider::for_param(&params.slider_1_skew, setter).with_width(200.0));
                                });
                                ui.horizontal(|ui|{
                                    ui.add(widgets::ParamSlider::for_param(&params.slider_2_coeff, setter).with_width(200.0));
                                    ui.add(widgets::ParamSlider::for_param(&params.slider_2_skew, setter).with_width(200.0));
                                });
                            });
                        });
                    });
                }
            )
    }

    

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // After `PEAK_METER_DECAY_MS` milliseconds of pure silence, the peak meter's value should
        // have dropped by 12 dB
        self.out_meter_decay_weight = 0.25f64.powf((buffer_config.sample_rate as f64 * PEAK_METER_DECAY_MS / 1000.0).recip()) as f32;

        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for mut channel_samples in buffer.iter_samples() {
            let mut out_amplitude: f32 = 0.0;
            let mut in_amplitude: f32 = 0.0;
            let mut processed_sample_l: f32;
            let mut processed_sample_r: f32;
            let num_samples = channel_samples.len();

            let gain: f32 = util::gain_to_db(self.params.free_gain.smoothed.next());
            let num_gain: f32;
            let output_gain: f32 = self.params.output_gain.smoothed.next();
            let slider_1_coeff: f32 = self.params.slider_1_coeff.smoothed.next();
            let slider_1_skew: f32 = self.params.slider_1_skew.smoothed.next();
            let slider_2_coeff: f32 = self.params.slider_2_coeff.smoothed.next();
            let slider_2_skew: f32 = self.params.slider_2_skew.smoothed.next();
            let push_amount: f32 = self.params.push_amount.smoothed.next();
            let dry_wet: f32 = self.params.dry_wet.value();

            // Split left and right same way original subhoofer did
            let mut in_l = *channel_samples.get_mut(0).unwrap();
            let mut in_r = *channel_samples.get_mut(1).unwrap();

            num_gain = gain;
            in_l *= util::db_to_gain(num_gain);
            in_r *= util::db_to_gain(num_gain);
            in_amplitude += in_l + in_r;

            ///////////////////////////////////////////////////////////////////////
            // Perform processing on the sample

            // Normalize really small values
            if in_l.abs() < 1.18e-23 { in_l = 0.1 * 1.18e-17; }
            if in_r.abs() < 1.18e-23 { in_r = 0.1 * 1.18e-17; }

            // Calculate our sin 'warmed' sample
            processed_sample_l = (1.0 - push_amount)*in_l + push_amount*(in_l.sin());
            processed_sample_r = (1.0 - push_amount)*in_r + push_amount*(in_r.sin());

            self.left_vec.push_front(processed_sample_l);
            self.left_vec.pop_back();
            self.right_vec.push_front(processed_sample_r);
            self.right_vec.pop_back();

            let mut temp_l: f32 = 0.0;
            let mut temp_r: f32 = 0.0;

            /*
            // Summed
            for element in self.left_vec {
                temp_l += element * (slider_1_coeff + slider_1_skew*element.abs());
                temp_r += element * (slider_1_coeff + slider_1_skew*element.abs());
            }
            */

            // Sequential
            if true {
                temp_l += self.left_vec[0] * (slider_1_coeff + slider_1_skew*self.left_vec[0].abs());
                temp_l += self.left_vec[1] * (slider_2_coeff + slider_2_skew*self.left_vec[1].abs());

                temp_r += self.right_vec[0] * (slider_1_coeff + slider_1_skew*self.right_vec[0].abs());
                temp_r += self.right_vec[1] * (slider_2_coeff + slider_2_skew*self.right_vec[1].abs());
            }
            
            processed_sample_l = temp_l;
            processed_sample_r = temp_r;
            
            ///////////////////////////////////////////////////////////////////////

            // Calculate dry/wet mix
            let wet_gain: f32 = dry_wet;
            processed_sample_l = in_l + processed_sample_l * wet_gain;
            processed_sample_r = in_r + processed_sample_r * wet_gain;

            // get the output amplitude here
            processed_sample_l = processed_sample_l*output_gain;
            processed_sample_r = processed_sample_r*output_gain;
            out_amplitude += processed_sample_l + processed_sample_r;

            // Assign back so we can output our processed sounds
            *channel_samples.get_mut(0).unwrap() = processed_sample_l;
            *channel_samples.get_mut(1).unwrap() = processed_sample_r;

            // calculations that are only displayed on the GUI while the GUI is open
            if self.params.editor_state.is_open() {
                // Input gain meter
                in_amplitude = (in_amplitude / num_samples as f32).abs();
                let current_in_meter = self.in_meter.load(std::sync::atomic::Ordering::Relaxed);
                let new_in_meter = if in_amplitude > current_in_meter {in_amplitude} else {current_in_meter * self.out_meter_decay_weight + in_amplitude * (1.0 - self.out_meter_decay_weight)};
                self.in_meter.store(new_in_meter, std::sync::atomic::Ordering::Relaxed);

                // Output gain meter
                out_amplitude = (out_amplitude / num_samples as f32).abs();
                let current_out_meter = self.out_meter.load(std::sync::atomic::Ordering::Relaxed);
                let new_out_meter = if out_amplitude > current_out_meter {out_amplitude} else {current_out_meter * self.out_meter_decay_weight + out_amplitude * (1.0 - self.out_meter_decay_weight)};
                self.out_meter.store(new_out_meter, std::sync::atomic::Ordering::Relaxed);
            }
        }

        ProcessStatus::Normal
    }

    const MIDI_INPUT: MidiConfig = MidiConfig::None;

    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const HARD_REALTIME_ONLY: bool = false;

    fn task_executor(self: &Gain) -> TaskExecutor<Self> {
        // In the default implementation we can simply ignore the value
        Box::new(|_| ())
    }

    fn filter_state(_state: &mut PluginState) {}

    fn reset(&mut self) {}

    fn deactivate(&mut self) {}
}

impl ClapPlugin for Gain {
    const CLAP_ID: &'static str = "com.ardura.gladedesk";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Custom Console Idea");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Gain {
    const VST3_CLASS_ID: [u8; 16] = *b"GladeDeskArduraA";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_clap!(Gain);
nih_export_vst3!(Gain);
