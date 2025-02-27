extern crate ffmpeg_next as ffmpeg;

use codec::context::Context;
use ffmpeg_next::codec::traits::Encoder;
use ffmpeg_next::format::context::Output;
use ffmpeg_next::{
    codec, decoder, encoder, format, frame, picture, Dictionary, Frame, Packet, Rational,
};
use std::time::Instant;

pub const DEFAULT_X264_OPTS: &str = "preset=medium";

pub struct VideoTranscoder {
    ost_index: usize,
    pub(crate) decoder: decoder::Video,
    input_time_base: Rational,
    pub(crate) encoder: encoder::Video,
    logging_enabled: bool,
    frame_count: usize,
    last_log_frame_count: usize,
    starting_time: Instant,
    last_log_time: Instant,
}

pub struct SubtitleTranscoder {
    ost_index: usize,
    pub(crate) decoder: decoder::Subtitle,
    input_time_base: Rational,
    encoder: encoder::Subtitle,
    logging_enabled: bool,
    frame_count: usize,
    last_log_frame_count: usize,
    starting_time: Instant,
    last_log_frame: Instant,
}

pub trait Transcoder {
    fn new(
        ist: &format::stream::Stream,
        octx: &mut Output,
        ost_index: usize,
        x264_opts: Dictionary,
        enable_logging: bool,
    ) -> Result<Self, ffmpeg::Error>
    where
        Self: Sized;
    fn send_frame_to_encoder(&mut self, frame: &Frame);
    fn send_packet_to_decoder(&mut self, packet: &Packet);
    fn send_eof_to_decoder(&mut self);
    fn receive_and_process_encoded_packets(&mut self, octx: &mut Output, ost_time_base: Rational);
    fn receive_and_process_decoded_frames(&mut self, octx: &mut Output, ost_time_base: Rational);
    fn log_progress(&mut self, timestamp: f64);
}

impl Transcoder for SubtitleTranscoder {
    fn new(
        ist: &format::stream::Stream,
        octx: &mut Output,
        ost_index: usize,
        x264_opts: Dictionary,
        enable_logging: bool,
    ) -> Result<Self, ffmpeg::Error> {
        let decoder = Context::from_parameters(ist.parameters())?
            .decoder()
            .subtitle()?;

        dbg!(&decoder.id());

        let codec = encoder::find(codec::Id::ASS).ok_or(ffmpeg::Error::EncoderNotFound)?;

        let mut ost = octx.add_stream(codec)?;

        dbg!("before encoder");

        let mut encoder = Context::new_with_codec(codec).encoder().subtitle()?;

        dbg!("after encoder");

        ost.set_parameters(&encoder);

        dbg!("after setting parameters for encoder");

        dbg!(decoder.time_base());
        dbg!(decoder.frame_rate());

        encoder.set_frame_rate(decoder.frame_rate());

        encoder.set_time_base(Rational(1, 1000));

        let opened_encoder = encoder.open()?;
        ost.set_parameters(&opened_encoder);

        dbg!("after setting output to encoder");

        Ok(Self {
            ost_index,
            decoder,
            input_time_base: ist.time_base(),
            encoder: opened_encoder,
            logging_enabled: enable_logging,
            frame_count: 0,
            last_log_frame_count: 0,
            starting_time: Instant::now(),
            last_log_frame: Instant::now(),
        })
    }

    fn send_frame_to_encoder(&mut self, frame: &Frame) {
        self.encoder.send_frame(frame).unwrap();
    }

    fn send_packet_to_decoder(&mut self, packet: &Packet) {
        self.decoder.send_packet(packet).unwrap();
    }

    fn send_eof_to_decoder(&mut self) {
        self.encoder.send_eof().unwrap();
    }

    fn receive_and_process_encoded_packets(&mut self, octx: &mut Output, ost_time_base: Rational) {
        let mut encoded = Packet::empty();

        while self.encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(self.ost_index);
            encoded.rescale_ts(self.input_time_base, ost_time_base);
            encoded.write_interleaved(octx).unwrap()
        }
    }

    fn receive_and_process_decoded_frames(&mut self, octx: &mut Output, ost_time_base: Rational) {
        todo!()
    }

    fn log_progress(&mut self, timestamp: f64) {
        todo!()
    }
}

impl Transcoder for VideoTranscoder {
    fn new(
        ist: &format::stream::Stream,
        octx: &mut Output,
        ost_index: usize,
        x264_opts: Dictionary,
        enable_logging: bool,
    ) -> Result<Self, ffmpeg::Error> {
        // On vérifie s'il y a des headers Globaux (commun sur le x264).
        let global_header = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);
        // Chercher le decoder
        let decoder = Context::from_parameters(ist.parameters())?
            .decoder()
            .video()?;

        // Chercher le codec h264
        let codec = encoder::find(codec::Id::H264);
        // Ajouter ce codec à l'output stream
        let mut ost = octx.add_stream(codec)?;

        // Initialiser l'encoder avec le codec video h264
        let mut encoder = Context::new_with_codec(codec.ok_or(ffmpeg::Error::InvalidData)?)
            .encoder()
            .video()?;

        // Paramétrer le stream output avec le bon encoder.
        ost.set_parameters(&encoder);
        // Ajouter les paramètres à l'encoder.
        encoder.set_height(decoder.height());
        encoder.set_width(decoder.width());
        encoder.set_aspect_ratio(decoder.aspect_ratio());
        encoder.set_format(decoder.format());
        encoder.set_frame_rate(decoder.frame_rate());
        encoder.set_time_base(ist.time_base());
        if global_header {
            encoder.set_flags(codec::Flags::GLOBAL_HEADER);
        }

        // Ouvrir l'encoder avec les options
        let opened_encoder = encoder
            .open_with(x264_opts)
            .expect("error opening encoder x264 with provided options");

        // Paramétrer l'encoder
        ost.set_parameters(&opened_encoder);
        // Initialiser le transcoder
        Ok(Self {
            ost_index,
            decoder,
            input_time_base: ist.time_base(),
            encoder: opened_encoder,
            logging_enabled: enable_logging,
            frame_count: 0,
            last_log_frame_count: 0,
            starting_time: Instant::now(),
            last_log_time: Instant::now(),
        })
    }

    fn send_frame_to_encoder(&mut self, frame: &Frame) {
        self.encoder.send_frame(frame).unwrap();
    }

    fn send_packet_to_decoder(&mut self, packet: &Packet) {
        self.decoder.send_packet(packet).unwrap()
    }

    fn send_eof_to_decoder(&mut self) {
        self.encoder.send_eof().unwrap();
    }

    //  Reads decoded frames, logs progress, and passes them to the encoder.
    fn receive_and_process_encoded_packets(
        &mut self,
        octx: &mut format::context::Output, // The output context where encoded packets are written
        ost_time_base: Rational, // The time base of the output stream (important for timestamp rescaling).
    ) {
        let mut encoded = Packet::empty();

        while self.encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(self.ost_index);
            encoded.rescale_ts(self.input_time_base, ost_time_base);
            encoded.write_interleaved(octx).unwrap();
        }
    }

    fn receive_and_process_decoded_frames(
        &mut self,
        octx: &mut format::context::Output, // The output context where encoded packets are written
        ost_time_base: Rational, // The time base of the output stream (important for timestamp rescaling).
    ) {
        let mut frame = frame::Video::empty();

        while self.decoder.receive_frame(&mut frame).is_ok() {
            self.frame_count += 1;
            let timestamp = frame.timestamp();
            self.log_progress(f64::from(
                Rational(timestamp.unwrap_or(0) as i32, 1) * self.input_time_base,
            ));
            frame.set_pts(timestamp);
            frame.set_kind(picture::Type::None);
            self.send_frame_to_encoder(&frame);
            self.receive_and_process_encoded_packets(octx, ost_time_base);
        }
    }

    fn log_progress(&mut self, timestamp: f64) {
        if !self.logging_enabled
            || (self.frame_count - self.last_log_frame_count < 100
                && self.last_log_time.elapsed().as_secs_f64() < 1.0)
        {
            return;
        }

        eprintln!(
            "time elapsed: \t{:8.2}\tframe count: {:8}\ttimestamp: {:8.2}",
            self.starting_time.elapsed().as_secs_f64(),
            self.frame_count,
            timestamp
        );

        self.last_log_frame_count = self.frame_count;
        self.last_log_time = Instant::now();
    }
}

pub fn parse_opts<'a>(s: String) -> Option<Dictionary<'a>> {
    let mut dict = Dictionary::new();
    for keyval in s.split_terminator(',') {
        let tokens: Vec<&str> = keyval.split('=').collect();
        match tokens[..] {
            [key, val] => dict.set(key, val),
            _ => return None,
        }
    }

    Some(dict)
}
