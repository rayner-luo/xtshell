use tokio_serial::{DataBits, Parity, SerialPortBuilderExt, StopBits};

fn main() {
    let ports = tokio_serial::available_ports().expect("available_ports");
    println!("== 共 {} 个串口 ==", ports.len());
    for p in &ports {
        let kind = match &p.port_type {
            tokio_serial::SerialPortType::UsbPort(info) => {
                format!("USB vid={:04x} pid={:04x} {}", info.vid, info.pid, info.product.clone().unwrap_or_default())
            }
            tokio_serial::SerialPortType::BluetoothPort => "Bluetooth".to_string(),
            tokio_serial::SerialPortType::PciPort => "PCI".to_string(),
            tokio_serial::SerialPortType::Unknown => "Unknown".to_string(),
        };
        println!("  {}  [{}]", p.port_name, kind);
    }

    let target = std::env::args().nth(1).unwrap_or_else(|| "COM3".into());
    let baud: u32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(9600);
    println!("== 尝试打开 {target} @ {baud} ==");

    println!("-- 默认配置:");
    match tokio_serial::new(&target, baud).open() {
        Ok(mut p) => {
            println!("   OK (default config)");
            println!("   baud={:?} data={:?} stop={:?} parity={:?} flow={:?}",
                p.baud_rate(), p.data_bits(), p.stop_bits(), p.parity(), p.flow_control());
        }
        Err(e) => println!("   FAIL: {e:?}"),
    }

    println!("-- 8N1 显式配置:");
    match tokio_serial::new(&target, baud)
        .data_bits(DataBits::Eight)
        .stop_bits(StopBits::One)
        .parity(Parity::None)
        .open()
    {
        Ok(_) => println!("   OK (8N1)"),
        Err(e) => println!("   FAIL: {e:?}"),
    }
}
