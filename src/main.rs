use std::{ffi::c_void, time::Duration};

use windows::{
  core::PCSTR,
  Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE},
    System::{
      Diagnostics::{
        Debug::{ReadProcessMemory, WriteProcessMemory},
        ToolHelp::{
          CreateToolhelp32Snapshot, Module32First, Module32Next, Process32First, Process32Next,
          MODULEENTRY32, PROCESSENTRY32, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
          TH32CS_SNAPPROCESS,
        },
      },
      Threading::{OpenProcess, PROCESS_ALL_ACCESS},
    },
    UI::WindowsAndMessaging::{FindWindowA, GetWindowThreadProcessId},
  },
};

use anyhow::{Context, Ok, Result};

// const LOCAL_PLAYER: usize = 0x509B74;
// const LOCAL_PLAYER: usize = 0x10F4F4;
const LOCAL_PLAYER: usize = 0x17B0B8;
const HEALTH_OFFSET: usize = 0xF8;
const RIFFLE_AMMO_OFFSET: usize = 0x150;
const RIFLE_AMMO_RESERVE_OFFSET: usize = 0x128;
const PISTOL_AMMO: usize = 0x13C;
const NAME_OFFSET: usize = 0x225;
const VEST_OFFSET: usize = 0xFC;

fn get_module_base_address(process_id: u32, module_name: &str) -> Result<Option<usize>> {
  unsafe {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, process_id)
      .context("error getting process snapshot")?;

    if snapshot.is_invalid() {
      anyhow::bail!("invalid snapshot handle: {:?}", snapshot);
    }

    let mut module_entry: MODULEENTRY32 = MODULEENTRY32::default();
    module_entry.dwSize = std::mem::size_of_val(&module_entry) as u32;

    if Module32First(snapshot, &mut module_entry as _).as_bool() {
      loop {
        let current_module_name = String::from_utf8(
          module_entry
            .szModule
            .into_iter()
            // Take characters until a '\0' is found
            .take_while(|character| character.0 != 0)
            .map(|character| character.0)
            .collect(),
        )?;

        if current_module_name == module_name {
          CloseHandle(snapshot);
          return Ok(Some(module_entry.modBaseAddr as usize));
        }

        if !Module32Next(snapshot, &mut module_entry as _).as_bool() {
          // No more modules to look at
          break;
        }
      }
    }

    CloseHandle(snapshot);
  }

  Ok(None)
}

fn get_process_id(process_name: &str) -> Result<Option<u32>> {
  unsafe {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
      .context("error getting processes snapshot")?;

    let mut process_entry = PROCESSENTRY32::default();
    process_entry.dwSize = std::mem::size_of_val(&process_entry) as u32;

    if !Process32First(snapshot, &mut process_entry as _).as_bool() {
      return Ok(None);
    }

    loop {
      let current_process_name = String::from_utf8(
        process_entry
          .szExeFile
          .into_iter()
          // Take characters until a '\0' is found
          .take_while(|character| character.0 != 0)
          .map(|character| character.0)
          .collect(),
      )?;

      if current_process_name == process_name {
        CloseHandle(snapshot);
        return Ok(Some(process_entry.th32ProcessID));
      }

      if !Process32Next(snapshot, &mut process_entry as _).as_bool() {
        // No more processes to look at
        break;
      }
    }

    CloseHandle(snapshot);
  }

  Ok(None)
}

fn follow_offsets(process_handle: HANDLE, base_address: usize, offsets: Vec<usize>) -> usize {
  let mut current_address = base_address;

  for offset in offsets.into_iter() {
    current_address += offset;

    println!(
      "following offset. base_address({:#x}) + offset=({:#x}) = {:x}",
      base_address, offset, current_address
    );

    unsafe {
      ReadProcessMemory(
        process_handle,
        current_address as *const usize as *mut c_void,
        &mut current_address as *mut _ as *mut c_void,
        std::mem::size_of_val(&current_address),
        std::ptr::null_mut(),
      )
      .expect("error reading process memory");
    }
  }

  current_address
}

fn main() -> Result<()> {
  let process_id = get_process_id("ac_client.exe")?.unwrap();

  println!("found process. process_id={process_id}");

  let process_handle = unsafe {
    OpenProcess(PROCESS_ALL_ACCESS, false, process_id).expect("unable to get process handle")
  };

  println!("got process handle. handle={process_handle:?}");

  let module_base = get_module_base_address(process_id, "ac_client.exe")?.unwrap();

  println!("got module base address. base_address={:#x}", module_base);

  unsafe {
    // let address = module_base + HEALTH_OFFSET;
    // let address = module_base + LOCAL_PLAYER;
    // health: 0x00901E5C <- 0x000000EC + 0x901D70 <- ac_client.exe+17E0A8 or 18AC00
    // 17E0A8 + 0x000000EC = 0x17E194
    // 0x300a83be1

    // let address = follow_offsets(process_handle, module_base + 0x18AC00, vec![0x000000EC]);
    // println!("{:#x}", address);
    let address = module_base + 0x17E0A8;

    // 0x730CD0

    let ptr_1 = {
      let mut output_buffer = 0;

      ReadProcessMemory(
        process_handle,
        address as _,
        &mut output_buffer as *mut _ as *mut c_void,
        std::mem::size_of_val(&output_buffer),
        std::ptr::null_mut(),
      )
      .expect("error reading process memory");

      output_buffer
    };

    println!("ptr_1={:#x}", ptr_1);

    let health_address = ptr_1 as usize + 0x000000EC;

    let health = {
      let mut output_buffer = 0;

      ReadProcessMemory(
        process_handle,
        health_address as _,
        &mut output_buffer as *mut _ as *mut c_void,
        std::mem::size_of_val(&output_buffer),
        std::ptr::null_mut(),
      )
      .expect("error reading process memory");

      output_buffer
    };
    dbg!(health);

    loop {
      let health_value = 69696969;
      println!("health_address={:#x}", health_address);
      WriteProcessMemory(
        process_handle,
        health_address as _,
        &health_value as *const i32 as *const c_void,
        std::mem::size_of_val(&health_value),
        std::ptr::null_mut(),
      )
      .expect("error writing process memory");
      std::thread::sleep(Duration::from_millis(100));
    }
  }

  return Ok(());

  let health = unsafe {
    let address: i32 = (LOCAL_PLAYER + HEALTH_OFFSET) as i32;

    println!("reading health at address: {address}");

    let mut output_buffer = 0;
    let mut bytes_read = 0;

    let result = ReadProcessMemory(
      process_handle,
      // &address as *const _ as *mut c_void,
      address as _,
      output_buffer as _,
      dbg!(std::mem::size_of_val(&output_buffer)),
      // &mut bytes_read as *mut u32,
      std::ptr::null_mut(),
    );
    dbg!(&result);
    dbg!(GetLastError());
    dbg!(bytes_read);
    result.unwrap();
    // .expect("error reading process memory");

    output_buffer
  };

  dbg!(health);

  Ok(())
}
