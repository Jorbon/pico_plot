use serialport::{SerialPort, SerialPortInfo};


pub fn find_port(cfg: &crate::config::Config, ports: Vec<SerialPortInfo>) -> Option<Box<dyn SerialPort>> {
	if ports.len() == 0 {
		println!("No serial port connections found.");
		return None;
	}
	
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
}

