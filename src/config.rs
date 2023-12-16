



#[derive(serde::Serialize, serde::Deserialize)]
pub struct Config {
	pub serial_number: String,
	pub manufacturer: String,
	pub time_between_samples: f64,
	pub time_zone_hour_offset: f64,
	pub default_max_picoamps: f64,
	pub default_min_picoamps: f64,
	pub default_window_time: f64,
	pub stroke_width: u32,
	pub window_width: u32,
	pub window_height: u32,
	pub always_on_top: bool
}

impl std::default::Default for Config {
    fn default() -> Self { Self {
		serial_number: String::from("CZA"),
		manufacturer: String::from("Prolific"),
		time_between_samples: 0.5,
		time_zone_hour_offset: -5.0,
		default_max_picoamps: 20.0,
		default_min_picoamps: -140.0,
		default_window_time: 300.0,
		stroke_width: 2,
		window_width: 960,
		window_height: 540,
		always_on_top: true
	} }
}

pub fn get_config() -> Config {
	match std::env::current_exe() {
		Ok(path) => confy::load_path(path.with_file_name("pico_plot_config.toml")).unwrap_or_else(|err| {
			println!("Couldn't access config file: {err}");
			Config::default()
		}),
		Err(err) => {
			println!("Couldn't locate config file: {err}");
			Config::default()
		}
	}
	
}

