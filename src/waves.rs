use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::{Receiver, Sender};

use rodio::dynamic_mixer::mixer;
use rodio::{default_output_device, Sink, Source};
use std::f32::consts::PI;
use std::sync::mpsc::sync_channel;
use std::thread::spawn;
use std::time::Duration;

#[allow(dead_code)]
pub mod notes {
    pub const C3: f32 = 130.81;
    pub const CS3: f32 = 138.59;
    pub const D3: f32 = 146.83;
    pub const DS3: f32 = 155.56;
    pub const E3: f32 = 164.81;
    pub const F3: f32 = 174.61;
    pub const FS3: f32 = 185.00;
    pub const G3: f32 = 196.00;
    pub const GS3: f32 = 207.65;
    pub const A3: f32 = 220.00;
    pub const AS3: f32 = 233.08;
    pub const B3: f32 = 246.94;
    pub const C4: f32 = 261.63;
    pub const CS4: f32 = 277.18;
    pub const D4: f32 = 293.66;
    pub const DS4: f32 = 311.13;
    pub const E4: f32 = 329.63;
    pub const F4: f32 = 349.23;
    pub const FS4: f32 = 369.99;
    pub const G4: f32 = 392.00;
    pub const GS4: f32 = 415.30;
    pub const A4: f32 = 440.00;
    pub const AS4: f32 = 466.16;
    pub const B4: f32 = 493.88;
    pub const C5: f32 = 523.25;
    pub const CS5: f32 = 554.37;
    pub const D5: f32 = 587.33;
    pub const DS5: f32 = 622.25;
    pub const E5: f32 = 659.25;
    pub const F5: f32 = 698.46;
    pub const FS5: f32 = 739.99;
    pub const G5: f32 = 783.00;
    pub const GS5: f32 = 830.61;
    pub const A5: f32 = 880.00;
    pub const AS5: f32 = 932.33;
    pub const B5: f32 = 987.77;
    pub const C6: f32 = 1046.50;
}

pub fn sine_wave(fraction_through: f32) -> f32 {
    (PI * 2.0 * fraction_through).sin()
}

pub fn square_wave(fraction_through: f32) -> f32 {
    if fraction_through < 0.5 {
        1.0
    } else {
        -1.0
    }
}

fn saw_wave(fraction_through: f32) -> f32 {
    fraction_through * 2.0 - 1.0
}

pub struct DynamicWave {
    frequency: f32,
    lamp: f32,
    ramp: f32,
    fraction_through: f32,
    step: f32,
    func: fn(f32) -> f32,
}

pub struct WaveUpdate {
    pub freq: f32,
    pub amp: (f32, f32),
}

impl DynamicWave {
    pub fn new(frequency: f32, amplitude: f32, func: fn(f32) -> f32) -> DynamicWave {
        DynamicWave {
            frequency,
            lamp: amplitude,
            ramp: amplitude,
            step: frequency / 48000.0,
            fraction_through: 0.0,
            func,
        }
    }
    fn update(&mut self, up: WaveUpdate) {
        self.frequency = up.freq;
        self.lamp = up.amp.0;
        self.ramp = up.amp.1;
        self.step = self.frequency / 48000.0;
    }
}

impl Iterator for DynamicWave {
    type Item = (f32, f32);
    fn next(&mut self) -> Option<Self::Item> {
        let part = (self.func)(self.fraction_through);
        let ans = (part * self.lamp, part * self.ramp);
        self.fraction_through += self.step;
        self.fraction_through %= 1.0;
        Some(ans)
    }
}

pub enum WaveCommand {
    Update(u64, WaveUpdate),
    Replace(u64, DynamicWave),
    Delete(u64),
}

pub struct CompositeWave {
    waves: HashMap<u64, DynamicWave>,
    sender: SyncSender<(f32, f32)>,
    command_reciever: Receiver<WaveCommand>,
}

pub struct WaveReceiver {
    r: Receiver<(f32, f32)>,
    pending_val: Option<f32>,
}

impl Iterator for WaveReceiver {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        match self.pending_val {
            Some(v) => {
                self.pending_val = None;
                Some(v)
            }
            None => {
                let (l, r) = self.r.recv().unwrap();
                self.pending_val = Some(r);
                Some(l)
            }
        }
    }
}

impl Source for WaveReceiver {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn channels(&self) -> u16 {
        2
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        48000
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl CompositeWave {
    fn update(&mut self, idx: u64, up: WaveUpdate) {
        self.waves.get_mut(&idx).unwrap().update(up);
    }
    fn replace(&mut self, idx: u64, w: DynamicWave) {
        self.waves.insert(idx, w);
    }
    fn delete(&mut self, idx: u64) {
        self.waves.remove(&idx);
    }

    pub fn generate(&mut self) {
        //First perform up to one edit of the waves we're generating
        let c = self.command_reciever.try_recv();
        if let Ok(wc) = c {
            match wc {
                WaveCommand::Update(idx, up) => self.update(idx, up),
                WaveCommand::Replace(idx, wave) => self.replace(idx, wave),
                WaveCommand::Delete(idx) => self.delete(idx),
            }
        }

        //Then send another iteration to the audio player
        let n = self.next().unwrap();
        self.sender.send(n).unwrap();
    }
}
impl Iterator for CompositeWave {
    type Item = (f32, f32);
    fn next(&mut self) -> Option<(f32, f32)> {
        if self.waves.len() > 0 {
            let (l, r) = self
                .waves
                .values_mut()
                .map(|w| w.next().unwrap())
                .fold((0.0, 0.0), |m, n| (m.0 + n.0, m.1 + n.1));
            Some((l / self.waves.len() as f32, r / self.waves.len() as f32))
        } else {
            Some((0.0, 0.0))
        }
    }
}

pub fn make_waves() -> Sender<WaveCommand> {
    let out = Sink::new(&default_output_device().unwrap());
    let (mix_in, mix_out) = mixer::<f32>(2, 48000);

    let (s, r) = sync_channel(1000);
    let (cs, cr) = channel();
    let mut cw = CompositeWave {
        waves: HashMap::new(),
        sender: s,
        command_reciever: cr,
    };

    spawn(move || loop {
        cw.generate();
    });

    mix_in.add(WaveReceiver {
        r,
        pending_val: None,
    });
    spawn(move || {
        out.append(mix_out);
        out.sleep_until_end();
    });

    cs
}

/*



let (mut cwave, cmdsender, r) = make_waves();





cmdsender.send(WaveCommand::Edit(1, w));

std::thread::sleep(std::time::Duration::from_millis(2000));
cmdsender.send(WaveCommand::Edit(2, DynamicWave::new(880.0, 1.0, sine_wave)));

std::thread::sleep(std::time::Duration::from_millis(2000));
cmdsender.send(WaveCommand::Edit(1, DynamicWave::new(440.0, 0.1, sine_wave)));




*/
