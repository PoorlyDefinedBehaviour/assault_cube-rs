use std::time::Duration;

use windows::{core::PCSTR, Win32::UI::WindowsAndMessaging::FindWindowA};

const LOCAL_PLAYER: usize = 0x509B74;
const HEALTH_OFFSET: usize = 0xF8;
const RIFFLE_AMMO_OFFSET: usize = 0x150;
const RIFLE_AMMO_RESERVE_OFFSET: usize = 0x128;
const PISTOL_AMMO: usize = 0x13C;
const NAME_OFFSET: usize = 0x225;
const VEST_OFFSET: usize = 0xFC;

fn main() {
  let game_window = loop {
    unsafe {
      let window = "AssaultCube";

      println!("waiting for {window}");

      let hwnd = FindWindowA(PCSTR(std::ptr::null()), window);
      // If the window was found
      if hwnd.0 != std::mem::zeroed() {
        println!("{window} found");
        break hwnd;
      }

      std::thread::sleep(Duration::from_secs(1));
    }
  };

  dbg!(game_window);
}
