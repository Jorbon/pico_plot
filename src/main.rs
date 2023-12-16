use std::collections::VecDeque;
use serialport::SerialPort;
use plotters::{prelude::*, backend::BGRXPixel};
use simple_windows::{SimpleWindowApp, WindowHandle, Rect};

mod find_port;
mod config;


fn main() {
	
	// List available serial ports
	
	let ports = serialport::available_ports().unwrap_or_else(|err| panic!("Could not get info on serial ports: {err}"));
	
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
	
	let cfg = config::get_config();
	let port = find_port::find_port(&cfg, ports);
	
	let app = App::new(&cfg, port);
	
	let result = simple_windows::run_window_process("pico plot", cfg.window_width, cfg.window_height, "Picoammeter Readings", cfg.always_on_top, app);
	
	match result {
		Ok(_) => {},
		Err(err) => println!("{err}")
	}
	
}






pub struct App {
	pub data: Vec<(f64, f64)>,
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
	pub data_color: RGBColor,
	pub data_stroke_width: u32,
	pub vertical_flip: bool
}

impl App {
	pub fn new(cfg: &config::Config, port: Option<Box<dyn SerialPort>>) -> Self {
		Self {
			data: vec![],
			x: 0.0,
			y: cfg.default_min_picoamps,
			w: cfg.default_window_time,
			h: cfg.default_max_picoamps - cfg.default_min_picoamps,
			mx: 0.0,
			my: 0.0,
			left_pad: 70.0,
			left_pad_min: 45.0,
			bottom_pad: 35.0,
			bottom_pad_min: 0.0,
			shift: false,
			ctrl: false,
			ml: false,
			mr: false,
			mm: false,
			port,
			min_sample_time: cfg.time_between_samples,
			previous_sample_time: std::time::Instant::now(),
			buffer: VecDeque::new(),
			program_start: std::time::Instant::now(),
			program_start_time_seconds: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs_f64() % 86400.0 + 3600.0 * cfg.time_zone_hour_offset,
			minimal: false,
			follow: true,
			bg_color: RGBColor(0, 0, 0),
			fg_color: RGBColor(255, 255, 255),
			data_color: RGBColor(255, 0, 0),
			data_stroke_width: cfg.stroke_width,
			vertical_flip: false
		}
	}
	
	pub fn x_scale_factor(&self, width: u32) -> f64 {
		self.w / (width as f64 - self.get_left_pad())
	}
	pub fn y_scale_factor(&self, height: u32) -> f64 {
		self.h / (height as f64 - self.get_bottom_pad())
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
	
	pub fn render_plot(&mut self, pixel_buffer: &mut [u8], width: u32, height: u32) -> Result<(), String> {
		let drawing_area = BitMapBackend::<BGRXPixel>::with_buffer_and_format(pixel_buffer, (width, height)).map_err(|err| err.to_string())?.into_drawing_area();
		
		drawing_area.fill(&self.bg_color).map_err(|err| err.to_string())?;
		
		let mut builder = ChartBuilder::on(&drawing_area);
		builder.x_label_area_size(match self.minimal { true => self.bottom_pad_min, false => self.bottom_pad });
		builder.y_label_area_size(match self.minimal { true => self.left_pad_min, false => self.left_pad });
		
		let mut chart = match self.vertical_flip {
			true => builder.build_cartesian_2d(self.x..(self.x + self.w), (self.y + self.h)..self.y).map_err(|err| err.to_string())?,
			false => builder.build_cartesian_2d(self.x..(self.x + self.w), self.y..(self.y + self.h)).map_err(|err| err.to_string())?
		};
		
		
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
		
		chart.draw_series(LineSeries::new(self.data.clone(), ShapeStyle::from(&self.data_color).stroke_width(self.data_stroke_width))).map_err(|err| err.to_string())?;
		drawing_area.present().map_err(|err| err.to_string())?;
		
		Ok(())
	}
}

impl SimpleWindowApp for App {
	fn on_paint(&mut self, handle: &WindowHandle, pixel_buffer: &mut [u8], client_rect: &Rect) {
		
		if let Some(ref mut _port) = self.port {
			// If a long time has passed without a request sent, the timer loop was broken and needs to be restarted
			let now = std::time::Instant::now();
			if now.duration_since(self.previous_sample_time).as_secs_f64() > self.min_sample_time * 10.0 {
				handle.set_timer(1, (self.min_sample_time * 1000.0) as u32);
			}
		}
		
		self.render_plot(pixel_buffer, client_rect.width() as u32, client_rect.height() as u32).unwrap_or_else(|err| println!("Couldn't display the plot: {err}"));
		
	}
	
	fn on_timer(&mut self, handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, timer_id: usize) {
		match timer_id {
			1 => {
				// Repeat this timer at the configured sample rate
				handle.set_timer(1, (self.min_sample_time * 1000.0) as u32);
				
				if let Some(ref mut port) = self.port {
					
					self.previous_sample_time = std::time::Instant::now();
					
					// Send data request
					if let Err(err) = port.write(b"MEAS:CURR:DC?\r") {
						println!("Error requesting measurement: {err}");
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
											let picoamps = amps * 1e12;
											self.data.push((time, picoamps));
											self.buffer.pop_front();
											i = 0;
											
											if self.follow {
												if time < self.x {
													self.x = time - 1.0;
												} else if time > self.x + self.w {
													self.x = time - self.w + 1.0;
												}
												if picoamps < self.y {
													self.h += self.y - picoamps;
													self.y = picoamps;
												} else if picoamps > self.y + self.h {
													self.h = picoamps - self.y;
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
			},
			_ => ()
		}
	}
	
	fn on_key_down(&mut self, handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, key_code: u32) {
		match key_code {
			16 => self.shift = true,
			17 => self.ctrl = true,
			73 => {
				self.minimal = !self.minimal;
				handle.request_redraw();
			}
			70 => self.follow = !self.follow,
			89 => {
				self.vertical_flip = !self.vertical_flip;
				handle.request_redraw();
			}
			68 => {
				self.bg_color = RGBColor(0, 0, 0);
				self.fg_color = RGBColor(255, 255, 255);
				handle.request_redraw();
			}
			76 => {
				self.bg_color = RGBColor(255, 255, 255);
				self.fg_color = RGBColor(0, 0, 0);
				handle.request_redraw();
			}
			83 => {
				self.bg_color = RGBColor(255, 239, 207);
				self.fg_color = RGBColor(0, 0, 0);
				handle.request_redraw();
			}
			32 => {
				let cfg = config::get_config();
				self.follow = true;
				self.w = cfg.default_window_time;
				self.h = cfg.default_max_picoamps - cfg.default_min_picoamps;
				self.y = cfg.default_min_picoamps;
				self.x = f64::max(0.0, self.data.last().unwrap_or(&(0.0, 0.0)).0 - self.w + 1.0);
				self.min_sample_time = cfg.time_between_samples;
				
				handle.request_redraw();
			}
			_ => ()
		}
	}
	
	fn on_key_up(&mut self, _handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, key_code: u32) {
		match key_code {
			16 => self.shift = false,
			17 => self.ctrl = false,
			_ => ()
		}
	}
	
	fn on_mouse_left_down(&mut self, _handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, _mouse_x: i16, _mouse_y: i16) {
		self.ml = true;
	}
	
	fn on_mouse_middle_down(&mut self, _handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, _mouse_x: i16, _mouse_y: i16) {
		self.mm = true;
	}
	
	fn on_mouse_right_down(&mut self, _handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, _mouse_x: i16, _mouse_y: i16) {
		self.mr = true;
	}
	
	fn on_mouse_left_up(&mut self, _handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, _mouse_x: i16, _mouse_y: i16) {
		self.ml = false;
	}
	
	fn on_mouse_middle_up(&mut self, _handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, _mouse_x: i16, _mouse_y: i16) {
		self.mm = false;
	}
	
	fn on_mouse_right_up(&mut self, _handle: &WindowHandle, _pixel_buffer: &mut [u8], _client_rect: &Rect, _mouse_x: i16, _mouse_y: i16) {
		self.mr = false;
	}
	
	
	
	fn on_mouse_move(&mut self, handle: &WindowHandle, _pixel_buffer: &mut [u8], client_rect: &Rect, mouse_x: i16, mouse_y: i16) {
		let dx = mouse_x as f64 - self.mx;
		let dy = mouse_y as f64 - self.my;
		
		self.mx = mouse_x as f64;
		self.my = mouse_y as f64;
		
		if self.ml || self.mr || self.mm {
			self.x -= dx * self.x_scale_factor(client_rect.width() as u32);
			self.y -= dy * self.y_scale_factor(client_rect.height() as u32) * match self.vertical_flip { true => 1.0, false => -1.0 };
			
			handle.request_redraw();
		}
	}
	
	fn on_scroll(&mut self, handle: &WindowHandle, _pixel_buffer: &mut [u8], client_rect: &Rect, scroll_distance: i16) {
		let distance = scroll_distance as f64 * 0.0005;
		
		if self.shift {
			self.x += distance * (self.mx - self.get_left_pad()) * self.x_scale_factor(client_rect.width() as u32);
			self.w *= 1.0 - distance;
		} else if self.ctrl {
			self.y += distance * match self.vertical_flip {
				true => self.my,
				false => client_rect.height() as f64 - self.my - self.get_bottom_pad()
			} * self.y_scale_factor(client_rect.height() as u32);
			self.h *= 1.0 - distance;
		} else {
			self.x -= distance * 500.0 * self.x_scale_factor(client_rect.width() as u32);
		}
		
		handle.request_redraw();
	}
}


