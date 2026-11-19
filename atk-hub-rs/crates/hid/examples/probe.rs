//! Read-only probe: lists COMPX mice, checks online state, prints a snapshot.
use atk_hid::compx::{Command, CompxMouse, Packet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(&atk_hid::list_mice()?)?);
    let (transport, _) = atk_hid::HidTransport::open_first_mouse()?;
    let mut mouse = CompxMouse::new(transport);
    let online = mouse.transfer(Packet::new(Command::GetWirelessMouseOnline))?;
    println!("online reply {:?}", online.0);
    println!("{}", serde_json::to_string_pretty(&mouse.snapshot()?)?);
    Ok(())
}
