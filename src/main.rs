use std::collections::VecDeque;

use serialport::SerialPort;
use speedy2d::{window::{WindowCreationOptions, WindowSize, WindowPosition, WindowHandler, WindowHelper, MouseScrollDistance, VirtualKeyCode, KeyScancode, MouseButton}, dimen::{Vector2, Vec2, UVec2}, Window, Graphics2D, image::{ImageDataType, ImageSmoothingMode}};
use plotters::prelude::*;


fn main() {
	
	
	// List available serial ports
	
	let ports = serialport::available_ports().unwrap_or_else(|err| panic!("Could not get info on serial ports: {err}"));
	let n = ports.len();
	
	for p in ports.clone() {
		println!("Name: {}", p.port_name);
		match p.port_type {
			serialport::SerialPortType::UsbPort(info) => {
				println!("Type: USB\nVID: {:04x} PID: {:04x}", info.vid, info.pid);
				println!("Serial number: {}", info.serial_number.unwrap_or(String::from("N/A")));
				println!("Manufacturer: {}", info.manufacturer.unwrap_or(String::from("N/A")));
				println!("Product: {}", info.product.unwrap_or(String::from("N/A")));
			}
			serialport::SerialPortType::BluetoothPort => println!("Type: Bluetooth"),
			serialport::SerialPortType::PciPort => println!("Type: PCI"),
			serialport::SerialPortType::Unknown => println!("Type: Unknown")
		}
		
		println!();
	}
	
	let cfg = get_config();
	
	let size = Vector2 { x: cfg.window_width as f32, y: cfg.window_height as f32 };
	let options = WindowCreationOptions::new_windowed(WindowSize::ScaledPixels(size), Some(WindowPosition::Center)).with_vsync(true);
	let window = Window::new_with_options("Picoammeter Readings", options).unwrap_or_else(|err| panic!("Could not create a plot window: {err}"));
	
	let w = MyWindowHandler::new(size, &cfg, if n == 0 {
		println!("No serial port connections found.");
		None
		
	} else {
		
		let mut port_name = None;
		
		// Search for matching serial number
		for p in &ports {
			if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
				if let Some(sn) = &info.serial_number {
					if *sn == cfg.serial_number {
						port_name = Some(&p.port_name);
						println!("Using port {} with matching serial number '{}'", p.port_name, *sn);
						break;
					}
				}
			}
		}
		
		// Next search for matching manufacturer
		if let None = port_name {
			for p in &ports {
				if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
					if let Some(m) = &info.manufacturer {
						if *m == cfg.manufacturer {
							port_name = Some(&p.port_name);
							println!("No serial number match, using port {} with matching manufacturer '{}'", p.port_name, *m);
							break;
						}
					}
				}
			}
		}
		
		// Next use the first usb connection
		if let None = port_name {
			for p in &ports {
				if let serialport::SerialPortType::UsbPort(_info) = &p.port_type {
					port_name = Some(&p.port_name);
					println!("No serial number or manufacturer match, using port {}", p.port_name);
					break;
				}
			}
		}
		
		let port_attempt = serialport::new(port_name.unwrap_or(&ports[0].port_name), 9600)
		.stop_bits(serialport::StopBits::One)
		.data_bits(serialport::DataBits::Eight)
		.open();
		
		// Open port, just try the first port if none were selected
		match port_attempt {
			Ok(mut port) => {
				let commands = [
					"*RST",
					"ARM:COUN 1",
					"DISP:DIG 5",
					"SYST:ZCH OFF",
					"SENS:CURR:NPLC 6",
					"FORM:ELEM READ",
					"TRIG:COUN 1"
				];
				
				let mut port_setup = true;
				
				for command in commands {
					let mut c = String::from(command);
					c.push('\r');
					match port.write(c.as_bytes()) {
						Ok(_) => (),
						Err(_err) => {
							println!("Couldn't send command: {command}");
							port_setup = false;
							break;
						}
					};
					std::thread::sleep(std::time::Duration::from_millis(50));
				}
				
				match port_setup {
					true => Some(port),
					false => None
				}
			}
			Err(_) => {
				println!("Could not open selected port {}", port_name.unwrap_or(&ports[0].port_name));
				None
			}
		}
		
		
	});
	
	
	
	
	window.run_loop(w);
	
}



#[derive(serde::Serialize, serde::Deserialize)]
pub struct Config {
	pub serial_number: String,
	pub manufacturer: String,
	pub time_between_samples: f64,
	pub time_zone_hour_offset: f64,
	pub default_max_picoamps: f64,
	pub default_min_picoamps: f64,
	pub default_window_time: f64,
	pub window_width: u32,
	pub window_height: u32
}

impl std::default::Default for Config {
    fn default() -> Self { Self {
		serial_number: String::from("CZA"),
		manufacturer: String::from("Prolific"),
		time_between_samples: 0.5,
		time_zone_hour_offset: -5.0,
		default_max_picoamps: 5.0,
		default_min_picoamps: -45.0,
		default_window_time: 300.0,
		window_width: 720,
		window_height: 405
	} }
}

pub fn get_config() -> Config {
	confy::load("pico_plot", "config").unwrap_or_else(|_err| {
		match confy::store("pico_plot", "config", Config::default()) {
			Ok(()) => println!("Config not found, generated new config file with default settings."),
			Err(_) => println!("Config not found and couldn't save a new config file, using default settings.")
		}
		Config::default()
	})
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
	pub left_pad: f64,
	pub left_pad_min: f64,
	pub bottom_pad: f64,
	pub bottom_pad_min: f64,
	pub shift: bool,
	pub ctrl: bool,
	pub alt: bool,
	pub ml: bool,
	pub mr: bool,
	pub mm: bool,
	pub port: Option<Box<dyn SerialPort>>,
	pub min_sample_time: f64,
	pub previous_sample_time: std::time::Instant,
	pub buffer: VecDeque<u8>,
	pub program_start: std::time::Instant,
	pub program_start_time_seconds: f64,
	pub minimal: bool,
	pub follow: bool,
	pub bg_color: RGBColor,
	pub fg_color: RGBColor,
	pub data_color: RGBColor
}

impl MyWindowHandler {
	pub fn new(size: Vector2<f32>, cfg: &Config, port: Option<Box<dyn SerialPort>>) -> Self {
		Self {
			size: (size.x as u32, size.y as u32),
			data: vec![],
			pixel_buffer: vec![0; (size.x * size.y * 3.0) as usize],
			x: 0.0,
			y: cfg.default_min_picoamps,
			w: cfg.default_window_time,
			h: cfg.default_max_picoamps - cfg.default_min_picoamps,
			mx: size.x as f64 * 0.5,
			my: size.y as f64 * 0.5,
			left_pad: 70.0,
			left_pad_min: 45.0,
			bottom_pad: 35.0,
			bottom_pad_min: 0.0,
			shift: false,
			ctrl: false,
			alt: false,
			ml: false,
			mr: false,
			mm: false,
			port,
			min_sample_time: cfg.time_between_samples,
			previous_sample_time: std::time::Instant::now(),
			buffer: VecDeque::new(),
			program_start: std::time::Instant::now(),
			program_start_time_seconds: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs_f64() % 86400.0 + 3600.0 * cfg.time_zone_hour_offset,
			minimal: true,
			follow: true,
			bg_color: RGBColor(0, 0, 0),
			fg_color: RGBColor(255, 255, 255),
			data_color: RGBColor(255, 0, 0)
		}
	}
	
	pub fn x_scale_factor(&self) -> f64 {
		self.w / (self.size.0 as f64 - self.get_left_pad())
	}
	pub fn y_scale_factor(&self) -> f64 {
		self.h / (self.size.1 as f64 - self.get_bottom_pad())
	}
	pub fn get_left_pad(&self) -> f64 {
		match self.minimal {
			true => self.left_pad_min,
			false => self.left_pad
		}
	}
	pub fn get_bottom_pad(&self) -> f64 {
		match self.minimal {
			true => self.bottom_pad_min,
			false => self.bottom_pad
		}
	}
	
	pub fn render_plot(&mut self) -> Result<(), String> {
		let drawing_area = BitMapBackend::with_buffer(&mut self.pixel_buffer, self.size).into_drawing_area();
		
		drawing_area.fill(&self.bg_color).map_err(|err| err.to_string())?;
		
		let mut chart = ChartBuilder::on(&drawing_area)
			.x_label_area_size(match self.minimal { true => self.bottom_pad_min, false => self.bottom_pad })
			.y_label_area_size(match self.minimal { true => self.left_pad_min, false => self.left_pad })
			//.margin(15)
			//.caption(&caption, ("sans-serif", 20).into_font())
			.build_cartesian_2d(self.x..(self.x + self.w), self.y..(self.y + self.h)).map_err(|err| err.to_string())?;
		
		match self.minimal {
			true => chart.configure_mesh()
				.y_label_formatter(&|y| format!("{}", y))
				.label_style(("sans-serif", 20, &self.fg_color))
				.axis_style(&self.fg_color)
				.bold_line_style(&self.fg_color.mix(0.2))
				.light_line_style(&self.fg_color.mix(0.1))
				.draw().map_err(|err| err.to_string())?,
			false => chart.configure_mesh()
				//.label_style(("sans-serif", 20))
				.x_label_formatter(&|x| {
					let seconds = (x + self.program_start_time_seconds) as u32;
					format!("{}:{:02}:{:02}", seconds / 3600, (seconds / 60) % 60, seconds % 60)
				})
				.y_label_formatter(&|y| format!("{}pA", y))
				.x_labels(10)
				.label_style(("sans-serif", 20, &self.fg_color))
				.axis_style(&self.fg_color)
				.bold_line_style(&self.fg_color.mix(0.2))
				.light_line_style(&self.fg_color.mix(0.1))
				//.axis_desc_style(("sans-serif", 25))
				//.x_desc("Time")
				//.y_desc("Beam current (pA)")
				.draw().map_err(|err| err.to_string())?
		}
		
		chart.draw_series(LineSeries::new(self.data.clone(), &self.data_color)).map_err(|err| err.to_string())?;
		drawing_area.present().map_err(|err| err.to_string())?;
		
		Ok(())
	}
}

impl WindowHandler for MyWindowHandler {
	fn on_draw(&mut self, helper: &mut WindowHelper, graphics: &mut Graphics2D) {
		
		if let Some(ref mut port) = self.port {
			// Request new data if enough time has passed
			let now = std::time::Instant::now();
			if now.duration_since(self.previous_sample_time).as_millis() > (1000.0 * self.min_sample_time) as u128 {
				if let Err(err) = port.write(b"MEAS:CURR:DC?\r") {
					println!("Error requesting measurement: {err}");
				};
				self.previous_sample_time = now;
			}
			
			// Store any data that has been received
			match port.bytes_to_read() {
				Ok(n) => {
					if n > 0 {
						let mut serial_buf = vec![0; n as usize];
						match port.read(&mut serial_buf) {
							Ok(_bytes) => self.buffer.append(&mut VecDeque::from(serial_buf)),
							Err(err) => println!("Couldn't read received data: {err}")
						}
					}
				}
				Err(err) => println!("Couldn't read received data: {err}")
			}
			
			// Parse any complete data packages
			let mut i = 0;
			while i < self.buffer.len() {
				if self.buffer[i] == 13 {
					match String::from_utf8(self.buffer.drain(0..i).collect()) {
						Ok(s) => {
							match s.parse::<f64>() {
								Ok(amps) => {
									let time = self.program_start.elapsed().as_secs_f64();
									self.data.push((time, amps * 1e12));
									self.buffer.pop_front();
									i = 0;
									
									if self.follow {
										if time < self.x {
											self.x = time - 1.0;
										} else if time > self.x + self.w {
											self.x = time - self.w + 1.0;
										}
									}
								}
								Err(err) => println!("Received data was in an unexpected format: {err}")
							}
						}
						Err(err) => println!("Received data was in an unexpected format: {err}")
					}
					
				} else {
					i += 1;
				}
			}
		}
		
		
		match self.render_plot() {
			Ok(()) => {
				match graphics.create_image_from_raw_pixels(ImageDataType::RGB, ImageSmoothingMode::Linear, self.size, &self.pixel_buffer) {
					Ok(image) => graphics.draw_image((0.0, 0.0), &image),
					Err(err) => println!("Couldn't display the plot: {err}")
				}
			}
			Err(err) => println!("Couldn't display the plot: {err}")
		}
		
		
		
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
			VirtualKeyCode::I => self.minimal = !self.minimal,
			VirtualKeyCode::F => self.follow = !self.follow,
			VirtualKeyCode::D => {
				self.bg_color = RGBColor(0, 0, 0);
				self.fg_color = RGBColor(255, 255, 255);
			}
			VirtualKeyCode::L => {
				self.bg_color = RGBColor(255, 255, 255);
				self.fg_color = RGBColor(0, 0, 0);
			}
			VirtualKeyCode::S => {
				self.bg_color = RGBColor(255, 239, 207);
				self.fg_color = RGBColor(0, 0, 0);
			}
			VirtualKeyCode::Space => {
				let cfg = get_config();
				self.follow = true;
				self.w = cfg.default_window_time;
				self.h = cfg.default_max_picoamps - cfg.default_min_picoamps;
				self.y = cfg.default_min_picoamps;
				self.x = f64::max(0.0, self.data.last().unwrap_or(&(0.0, 0.0)).0 - self.w + 1.0);
				self.min_sample_time = cfg.time_between_samples;
			}
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
			self.x += d * 0.0025 * (self.mx - self.get_left_pad()) * self.x_scale_factor();
			self.w *= 1.0 - d * 0.0025;
		} else if self.ctrl {
			self.y += d * 0.0025 * (self.size.1 as f64 - self.my - self.get_bottom_pad()) * self.y_scale_factor();
			self.h *= 1.0 - d * 0.0025;
		} else {
			self.x -= d * self.x_scale_factor();
		}
		
	}
	
	
}


