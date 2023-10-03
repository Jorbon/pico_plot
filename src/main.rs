use std::collections::VecDeque;

use serialport::SerialPort;
use speedy2d::{window::{WindowCreationOptions, WindowSize, WindowPosition, WindowHandler, WindowHelper, MouseScrollDistance, VirtualKeyCode, KeyScancode, MouseButton}, dimen::{Vector2, Vec2, UVec2}, Window, Graphics2D, image::{ImageDataType, ImageSmoothingMode}};
use plotters::prelude::*;


fn main() {
	
	// List available serial ports
	
	let ports = serialport::available_ports().unwrap();
	let n = ports.len();
	
	for p in ports.clone() {
		println!("Name: {}", p.port_name);
		match p.port_type {
			serialport::SerialPortType::UsbPort(info) => {
				println!("Type: USB\nVID: {:04x} PID: {:04x}", info.vid, info.pid);
				println!("Serial number: {}", info.serial_number.unwrap_or(String::new()));
				println!("Manufacturer: {}", info.manufacturer.unwrap_or(String::new()));
				println!("Product: {}", info.product.unwrap_or(String::new()));
			}
			serialport::SerialPortType::BluetoothPort => println!("Type: Bluetooth"),
			serialport::SerialPortType::PciPort => println!("Type: PCI"),
			serialport::SerialPortType::Unknown => println!("Type: Unknown")
		}
		
		println!();
	}
	
	if n == 0 {
		println!("No serial ports connected.");
		return;
	}
	
	
	
	// setup picoammeter
	
	//zero check
	//units: nA -> Sci
	
	let mut port = serialport::new(&ports[0].port_name, 9600)
		.stop_bits(serialport::StopBits::One)
		.data_bits(serialport::DataBits::Eight)
		.open()
		.unwrap();
	
	let commands = [
		"*RST",
		"ARM:COUN 1",
		"DISP:DIG 5",
		"SYST:ZCH OFF",
		"SENS:CURR:NPLC 6",
		"FORM:ELEM READ",
		"TRIG:COUN 1"
	];
	
	for command in commands {
		port.write(command.as_bytes()).unwrap();
		port.write(b"\r").unwrap();
		std::thread::sleep(std::time::Duration::from_millis(50));
	}
	
	
	// start loop
	
	let size = Vector2 {x: 720.0, y: 405.0};
	let options = WindowCreationOptions::new_windowed(WindowSize::ScaledPixels(size), Some(WindowPosition::Center)).with_vsync(true);
	let window = Window::new_with_options("Picoammeter Readings", options).unwrap();
	let w = MyWindowHandler::new(size, port);
	
	window.run_loop(w);
	
}


pub struct MyWindowHandler {
	pub size: (u32, u32),
	pub data: Vec<(f64, f64)>,
	pub pixel_buffer: Vec<u8>,
	pub x: f64,
	pub y: f64,
	pub w: f64,
	pub h: f64,
	pub mx: f64,
	pub my: f64,
	pub pad: f64,
	pub left_pad: f64,
	pub bottom_pad: f64,
	pub top_pad: f64,
	pub right_pad: f64,
	pub shift: bool,
	pub ctrl: bool,
	pub alt: bool,
	pub ml: bool,
	pub mr: bool,
	pub mm: bool,
	pub port: Box<dyn SerialPort>,
	pub min_sample_time: f64,
	pub previous_sample_time: std::time::Instant,
	pub buffer: VecDeque<u8>,
	pub program_start: std::time::Instant
}

impl MyWindowHandler {
	pub fn new(size: Vector2<f32>, port: Box<dyn SerialPort>) -> Self {
		Self {
			size: (size.x as u32, size.y as u32),
			data: vec![(0.0, 0.0), (0.0, 0.1)],
			pixel_buffer: vec![0; (size.x * size.y * 3.0) as usize],
			x: 0.0,
			y: 0.0,
			w: 100.0,
			h: 10.0,
			mx: size.x as f64 * 0.5,
			my: size.y as f64 * 0.5,
			pad: 15.0,
			left_pad: 40.0,
			bottom_pad: 40.0,
			top_pad: 0.0,
			right_pad: 0.0,
			shift: false,
			ctrl: false,
			alt: false,
			ml: false,
			mr: false,
			mm: false,
			port,
			min_sample_time: 0.2,
			previous_sample_time: std::time::Instant::now(),
			buffer: VecDeque::new(),
			program_start: std::time::Instant::now()
		}
	}
	
	pub fn x_scale_factor(&self) -> f64 {
		self.w / (self.size.0 as f64 - self.pad * 2.0 - self.left_pad - self.right_pad)
	}
	pub fn y_scale_factor(&self) -> f64 {
		self.h / (self.size.1 as f64 - self.pad * 2.0 - self.top_pad - self.bottom_pad)
	}
}

impl WindowHandler for MyWindowHandler {
	fn on_draw(&mut self, helper: &mut WindowHelper, graphics: &mut Graphics2D) {
		
		// Request new data if enough time has passed
		let now = std::time::Instant::now();
		if now.duration_since(self.previous_sample_time).as_millis() > (1000.0 * self.min_sample_time) as u128 {
			self.port.write(b"MEAS:CURR:DC?\r").unwrap();
			self.previous_sample_time = now;
		}
		
		// Store any data that has been received
		let n = self.port.bytes_to_read().unwrap();
		if n > 0 {
			let mut serial_buf = vec![0; n as usize];
			self.port.read(&mut serial_buf).unwrap();
			self.buffer.append(&mut VecDeque::from(serial_buf));
		}
		
		// Parse any complete data packages
		let mut i = 0;
		while i < self.buffer.len() {
			if self.buffer[i] == 13 {
				let value = String::from_utf8(self.buffer.drain(0..i).collect()).unwrap().parse::<f64>().unwrap();
				self.data.push((self.program_start.elapsed().as_secs_f64(), value * 1e12));
				
				self.buffer.pop_front();
				i = 0;
			} else {
				i += 1;
			}
		}
		
		
		{
			let drawing_area = BitMapBackend::with_buffer(&mut self.pixel_buffer, self.size).into_drawing_area();
			drawing_area.fill(&WHITE).unwrap();
			
			let mut chart = ChartBuilder::on(&drawing_area)
				.x_label_area_size(self.bottom_pad)
				.y_label_area_size(self.left_pad)
				.margin(self.pad)
				//.caption(&caption, ("sans-serif", 20).into_font())
				.build_cartesian_2d(self.x..(self.x + self.w), self.y..(self.y + self.h)).unwrap();
			
			chart.configure_mesh()
				//.label_style(("sans-serif", 20))
				.axis_desc_style(("sans-serif", 16))
				.x_desc("Time (s)")
				.y_desc("Beam current (pA)")
				.draw().unwrap();
			
			chart.draw_series(LineSeries::new(self.data.clone(), &RED)).unwrap();
			
			//chart.configure_series_labels()
				//.label_font(("sans-serif", 20).into_font())
				//.background_style(&WHITE.mix(0.8))
				//.border_style(&BLACK)
				//.draw().unwrap();
			
			drawing_area.present().unwrap();
		}
		
		let image = graphics.create_image_from_raw_pixels(ImageDataType::RGB, ImageSmoothingMode::Linear, self.size, &self.pixel_buffer).unwrap();
		graphics.draw_image((0.0, 0.0), &image);
		
		helper.request_redraw();
	}
	
	fn on_resize(&mut self, _helper: &mut WindowHelper<()>, size_pixels: UVec2) {
		self.size = (size_pixels.x, size_pixels.y);
		self.pixel_buffer = vec![0; (size_pixels.x * size_pixels.y * 3) as usize];
	}
	
	fn on_key_down(&mut self, _helper: &mut WindowHelper<()>, virtual_key_code: Option<VirtualKeyCode>, _scancode: KeyScancode) {
		if let Some(keycode) = virtual_key_code { match keycode {
			VirtualKeyCode::RShift | VirtualKeyCode::LShift => self.shift = true,
			VirtualKeyCode::RControl | VirtualKeyCode::LControl => self.ctrl = true,
			VirtualKeyCode::RAlt | VirtualKeyCode::LAlt => self.alt = true,
			_ => ()
		}}
	}
	
	fn on_key_up(&mut self, _helper: &mut WindowHelper<()>, virtual_key_code: Option<VirtualKeyCode>, _scancode: KeyScancode) {
		if let Some(keycode) = virtual_key_code { match keycode {
			VirtualKeyCode::RShift | VirtualKeyCode::LShift => self.shift = false,
			VirtualKeyCode::RControl | VirtualKeyCode::LControl => self.ctrl = false,
			VirtualKeyCode::RAlt | VirtualKeyCode::LAlt => self.alt = false,
			_ => ()
		}}
	}
	
	fn on_mouse_button_down(&mut self, _helper: &mut WindowHelper<()>, button: MouseButton) {
		match button {
			MouseButton::Left => self.ml = true,
			MouseButton::Right => self.mr = true,
			MouseButton::Middle => self.mm = true,
			MouseButton::Other(_) => ()
		}
	}
	
	fn on_mouse_button_up(&mut self, _helper: &mut WindowHelper<()>, button: MouseButton) {
		match button {
			MouseButton::Left => self.ml = false,
			MouseButton::Right => self.mr = false,
			MouseButton::Middle => self.mm = false,
			MouseButton::Other(_) => ()
		}
	}
	
	fn on_mouse_move(&mut self, _helper: &mut WindowHelper<()>, position: Vec2) {
		let dx = position.x as f64 - self.mx;
		let dy = position.y as f64 - self.my;
		
		self.mx = position.x as f64;
		self.my = position.y as f64;
		
		if self.ml || self.mr || self.mm {
			self.x -= dx * self.x_scale_factor();
			self.y += dy * self.y_scale_factor();
		}
	}
	
	fn on_mouse_wheel_scroll(&mut self, _helper: &mut WindowHelper<()>, distance: MouseScrollDistance) {
		let d = match distance {
			MouseScrollDistance::Pixels { x: _, y, z: _ } => y,
			MouseScrollDistance::Lines { x: _, y, z: _ } => y * 20.0,
			MouseScrollDistance::Pages { x: _, y, z: _ } => y * 100.0
		};
		
		if self.shift {
			self.x -= d * self.x_scale_factor();
		} else if self.ctrl {
			self.x += d * 0.0025 * (self.mx - self.pad - self.left_pad) * self.x_scale_factor();
			self.w *= 1.0 - d * 0.0025;
		} else {
			self.x += d * 0.0025 * (self.mx - self.pad - self.left_pad) * self.x_scale_factor();
			self.y += d * 0.0025 * (self.size.1 as f64 - self.my - self.pad - self.bottom_pad) * self.y_scale_factor();
			self.w *= 1.0 - d * 0.0025;
			self.h *= 1.0 - d * 0.0025;
		}
		
	}
	
	
}



