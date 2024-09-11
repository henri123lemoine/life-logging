use plotters::prelude::*;
use plotters::backend::RGBPixel;
use tracing::info;

pub struct AudioVisualizer;

impl AudioVisualizer {
    pub fn create_waveform(data: &[f32], width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; (width * height * 3) as usize];
        {
            let root = BitMapBackend::<RGBPixel>::with_buffer_and_format(
                &mut buffer,
                (width, height)
            ).unwrap().into_drawing_area();
            
            root.fill(&WHITE).unwrap();

            let mut chart = ChartBuilder::on(&root)
                .build_cartesian_2d(0f32..data.len() as f32, -1f32..1f32)
                .unwrap();

            chart
                .configure_mesh()
                .disable_x_mesh()
                .disable_y_mesh()
                .draw()
                .unwrap();

            chart
                .draw_series(LineSeries::new(
                    data.iter().enumerate().map(|(i, &v)| (i as f32, v)),
                    &RED,
                ))
                .unwrap();

            root.present().unwrap();
        }

        // Convert RGB buffer to PNG
        let mut png_data = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_data, width, height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&buffer).unwrap();
        }

        info!("Generated waveform visualization of {} bytes", png_data.len());
        png_data
    }
}
