use std::{ffi::c_void, time::Duration};

use windows::Win32::{
  Foundation::{CloseHandle, BOOL, HANDLE, HINSTANCE},
  System::{
    Console::{AllocConsole, FreeConsole},
    Diagnostics::{
      Debug::{ReadProcessMemory, WriteProcessMemory},
      ToolHelp::{
        CreateToolhelp32Snapshot, Module32First, Module32Next, Process32First, Process32Next,
        MODULEENTRY32, PROCESSENTRY32, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
      },
    },
    LibraryLoader::DisableThreadLibraryCalls,
    Threading::{OpenProcess, PROCESS_ALL_ACCESS},
  },
  UI::Input::KeyboardAndMouse::GetKeyState,
};

use anyhow::{Context, Ok, Result};

// const LOCAL_PLAYER: usize = 0x509B74;
// const LOCAL_PLAYER: usize = 0x10F4F4;
const LOCAL_PLAYER_OFFSET: usize = 0x0017E0A8;
const POSITION_X_FROM_LOCAL_PLAYER: usize = 0x4;
const POSITION_Y_FROM_LOCAL_PLAYER: usize = 0xC;
const SOMETHING_FROM_LOCAL_PLAYER: usize = 0x150;
const SOMETHING_FROM_LOCAL_PLAYER_2: usize = 0x164;
const SOMETHING_FROM_LOCAL_PLAYER_3: usize = 0x64;
const IS_PLAYER_ON_THE_FLOOR_FROM_LOCAL_PLAYER: usize = 0x5D;
const HEALTH_OFFSET_FROM_LOCAL_PLAYER: usize = 0xEC;
const RIFFLE_AMMO_OFFSET_FROM_LOCAL_PLAYER: usize = 0x140;
const PISTOL_AMMO_OFFSET_FROM_LOCAL_PLAYER: usize = 0x12C;
const GRENADE_OFFSET_FROM_LOCAL_PLAYER: usize = 0x144;

const ENTITY_LIST_OFFSET: usize = 0x18AC04;
const NAME_OFFSET: usize = 0x205;
const NUMBER_OF_PLAYERS_IN_MATCH_OFFSET: usize = 0x18AC0C;

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

fn follow_pointers(process_handle: HANDLE, initial_address: usize, offsets: Vec<usize>) -> usize {
  let mut current_address = initial_address;

  unsafe {
    if offsets.is_empty() {
      ReadProcessMemory(
        process_handle,
        current_address as *const usize as *mut c_void,
        &mut current_address as *mut _ as *mut c_void,
        std::mem::size_of_val(&current_address),
        std::ptr::null_mut(),
      )
      .expect("error reading process memory");
    } else {
      for offset in offsets.into_iter() {
        ReadProcessMemory(
          process_handle,
          current_address as *const usize as *mut c_void,
          &mut current_address as *mut _ as *mut c_void,
          std::mem::size_of_val(&current_address),
          std::ptr::null_mut(),
        )
        .expect("error reading process memory");

        current_address += offset;
      }
    }
  }

  current_address
}

fn main_old() -> Result<()> {
  let process_id = get_process_id("ac_client.exe")?.unwrap();

  println!("found process. process_id={process_id}");

  let process_handle = unsafe {
    OpenProcess(PROCESS_ALL_ACCESS, false, process_id).expect("unable to get process handle")
  };

  println!("got process handle. handle={process_handle:?}");

  let module_base_addr = get_module_base_address(process_id, "ac_client.exe")?.unwrap();

  println!(
    "got module base address. base_address={:#x}",
    module_base_addr
  );

  unsafe {
    let health_address = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![HEALTH_OFFSET_FROM_LOCAL_PLAYER],
    );

    println!("health_address={:#x}", health_address);

    let rifle_ammo_address = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![RIFFLE_AMMO_OFFSET_FROM_LOCAL_PLAYER],
    );

    println!("rifle_ammo_address={:#x}", rifle_ammo_address);

    let pistol_ammo_address = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![PISTOL_AMMO_OFFSET_FROM_LOCAL_PLAYER],
    );

    println!("pistol_ammo_address={:#x}", pistol_ammo_address);

    let grenade_address = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![GRENADE_OFFSET_FROM_LOCAL_PLAYER],
    );

    println!("grenade_address={:#x}", grenade_address);

    let position_x_address = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![POSITION_X_FROM_LOCAL_PLAYER],
    );

    println!("position_x_address={:#x}", position_x_address);

    let position_y_address = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![POSITION_Y_FROM_LOCAL_PLAYER],
    );

    println!("position_y_address={:#x}", position_y_address);

    let is_player_on_the_floor_address = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![IS_PLAYER_ON_THE_FLOOR_FROM_LOCAL_PLAYER],
    );

    println!(
      "is_player_on_the_floor_address={:#x}",
      is_player_on_the_floor_address
    );

    let something_address = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![SOMETHING_FROM_LOCAL_PLAYER],
    );

    println!("something_address={:#x}", something_address);

    let something_address_2 = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![SOMETHING_FROM_LOCAL_PLAYER_2],
    );

    println!("something_address_2={:#x}", something_address_2);

    let something_address_3 = follow_pointers(
      process_handle,
      module_base_addr + LOCAL_PLAYER_OFFSET,
      vec![0x24],
    );

    println!("something_address_3={:#x}", something_address_3);

    let num_players_in_match = {
      let num_players_in_match_address = module_base_addr + NUMBER_OF_PLAYERS_IN_MATCH_OFFSET;

      let mut buffer = 0_i32;

      ReadProcessMemory(
        process_handle,
        num_players_in_match_address as _,
        &mut buffer as *mut i32 as *mut c_void,
        std::mem::size_of_val(&buffer),
        std::ptr::null_mut(),
      )
      .expect("error reading process memory");

      // Remove 1 from the number of players in the match because we count as a player
      if buffer > 0 {
        (buffer - 1) as usize
      } else {
        0
      }
    };

    println!("match has {num_players_in_match} players");

    let address_of_entity_list_address = module_base_addr + 0x18AC04;
    println!(
      "address_of_entity_list_address={:#x}",
      address_of_entity_list_address
    );

    let mut entity_list_address = 0_i32;

    ReadProcessMemory(
      process_handle,
      address_of_entity_list_address as _,
      &mut entity_list_address as *mut i32 as *mut c_void,
      std::mem::size_of_val(&entity_list_address),
      std::ptr::null_mut(),
    )
    .expect("error reading process memory");

    println!("entity_list_address={:#x}", entity_list_address);

    // Skip the first entity list position because it is always empty.
    for i in 1..=num_players_in_match {
      let entity_address = entity_list_address as usize + i * 0x4;

      let name_address = follow_pointers(process_handle, entity_address, vec![NAME_OFFSET]) as u32;

      let mut buffer = [0_u8; 256];

      ReadProcessMemory(
        process_handle,
        name_address as _,
        &mut buffer as *mut [u8; 256] as *mut c_void,
        std::mem::size_of_val(&buffer),
        std::ptr::null_mut(),
      )
      .expect("error reading process memory");

      let name: Vec<u8> = buffer
        .into_iter()
        // Take characters until a '\0' is found
        .take_while(|character| *character != 0)
        .collect();

      println!(
        "player found at address {:#x}: {}",
        entity_address,
        String::from_utf8_lossy(&name)
      );
    }

    loop {
      let health_value = 69696969;

      WriteProcessMemory(
        process_handle,
        health_address as _,
        &health_value as *const i32 as *const c_void,
        std::mem::size_of_val(&health_value),
        std::ptr::null_mut(),
      )
      .expect("error writing process memory");

      let ammo_value = 123;

      WriteProcessMemory(
        process_handle,
        rifle_ammo_address as _,
        &ammo_value as *const i32 as *const c_void,
        std::mem::size_of_val(&ammo_value),
        std::ptr::null_mut(),
      )
      .expect("error writing process memory");

      WriteProcessMemory(
        process_handle,
        pistol_ammo_address as _,
        &ammo_value as *const i32 as *const c_void,
        std::mem::size_of_val(&ammo_value),
        std::ptr::null_mut(),
      )
      .expect("error writing process memory");

      let number_of_grenades = 100;

      WriteProcessMemory(
        process_handle,
        grenade_address as _,
        &number_of_grenades as *const i32 as *const c_void,
        std::mem::size_of_val(&number_of_grenades),
        std::ptr::null_mut(),
      )
      .expect("error writing process memory");

      // let something_value = 0;

      // WriteProcessMemory(
      //   process_handle,
      //   something_address as _,
      //   &something_value as *const i32 as *const c_void,
      //   std::mem::size_of_val(&something_value),
      //   std::ptr::null_mut(),
      // )
      // .expect("error writing process memory");

      // let something_value_2 = 0;

      // WriteProcessMemory(
      //   process_handle,
      //   something_address_2 as _,
      //   &something_value_2 as *const i32 as *const c_void,
      //   std::mem::size_of_val(&something_value_2),
      //   std::ptr::null_mut(),
      // )
      // .expect("error writing process memory");

      const CTRL_KEY: i32 = 0x11;

      if GetKeyState(CTRL_KEY) < 0 {
        let mut current_position = 0_f32;

        ReadProcessMemory(
          process_handle,
          position_y_address as _,
          &mut current_position as *mut f32 as *mut c_void,
          std::mem::size_of_val(&current_position),
          std::ptr::null_mut(),
        )
        .expect("error reading process memory");

        let new_position = 10.0_f32;

        println!("current_position={current_position} new_position={new_position}");

        let is_player_on_the_floor = 0;

        WriteProcessMemory(
          process_handle,
          is_player_on_the_floor_address as _,
          &is_player_on_the_floor as *const i32 as *const c_void,
          std::mem::size_of_val(&is_player_on_the_floor),
          std::ptr::null_mut(),
        )
        .expect("error writing process memory");

        WriteProcessMemory(
          process_handle,
          position_y_address as _,
          &new_position as *const f32 as *const c_void,
          std::mem::size_of_val(&new_position),
          std::ptr::null_mut(),
        )
        .expect("error writing process memory");

        let new_x_position = 200_f32;

        WriteProcessMemory(
          process_handle,
          position_x_address as _,
          &new_x_position as *const f32 as *const c_void,
          std::mem::size_of_val(&new_x_position),
          std::ptr::null_mut(),
        )
        .expect("error writing process memory");

        let something_value_3 = 1000000;

        WriteProcessMemory(
          process_handle,
          something_address_3 as _,
          &something_value_3 as *const i32 as *const c_void,
          std::mem::size_of_val(&something_value_3),
          std::ptr::null_mut(),
        )
        .expect("error writing process memory");
      }

      std::thread::sleep(Duration::from_millis(100));
    }
  }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
  dll_module: HINSTANCE,
  call_reason: u32,
  _reserved: *mut c_void,
) -> BOOL {
  const DLL_PROCESS_ATTACH: u32 = 1;

  if call_reason == DLL_PROCESS_ATTACH {
    unsafe {
      DisableThreadLibraryCalls(dll_module).expect("error disabling thread library calls");

      std::thread::spawn(|| {
        AllocConsole();
        loop {
          println!("injected. thread running inside process");
          std::thread::sleep(Duration::from_secs(3));
        }
      });
    }
  }

  BOOL::from(true)
}
